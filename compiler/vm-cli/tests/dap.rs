//! End-to-end handshake tests for the `ironplcvmd` Debug Adapter Protocol
//! server (Phase 4.3). These spawn the real binary and drive the
//! `initialize` → `launch` → `disconnect` path over stdin/stdout, asserting
//! the framed responses and events, plus the two launch-precondition failures
//! (`NoDebugInfo`, `MultiInstanceUnsupported`).
//!
//! These spawn `ironplcvmd`, so they also serve as the regression test for the
//! binary being built at all: it is no longer behind a feature gate, and a
//! change that stops building it stops this file from running.

use std::io::{Read, Write};
use std::process::{Command, Stdio};

use assert_cmd::cargo;
use ironplc_container::debug_section::{iec_type_tag, var_section, VarNameEntry};
use ironplc_container::{
    Container, ContainerBuilder, FunctionId, InstanceId, ProgramInstanceEntry, TaskEntry, TaskId,
    TaskType, VarIndex,
};
use serde_json::{json, Value};
use tempfile::NamedTempFile;

fn a_var_name() -> VarNameEntry {
    VarNameEntry {
        var_index: VarIndex::new(0),
        function_id: FunctionId::GLOBAL_SCOPE,
        var_section: var_section::VAR,
        iec_type_tag: iec_type_tag::DINT,
        name: "x".into(),
        type_name: "DINT".into(),
    }
}

fn a_task(task_id: TaskId) -> TaskEntry {
    TaskEntry {
        task_id,
        priority: 0,
        task_type: TaskType::Freewheeling,
        flags: 0x01,
        interval_us: 0,
        single_var_index: VarIndex::NO_SINGLE_VAR,
        watchdog_us: 0,
        input_image_offset: 0,
        output_image_offset: 0,
        reserved: [0; 4],
    }
}

fn a_program(instance_id: InstanceId, task_id: TaskId) -> ProgramInstanceEntry {
    ProgramInstanceEntry {
        instance_id,
        task_id,
        entry_function_id: FunctionId::new(0),
        var_table_offset: 0,
        var_table_count: 1,
        fb_instance_offset: 0,
        fb_instance_count: 0,
        init_function_id: FunctionId::new(0),
    }
}

fn write_container(container: &Container) -> NamedTempFile {
    let mut buf = Vec::new();
    container.write_to(&mut buf).unwrap();
    let mut file = NamedTempFile::new().unwrap();
    file.write_all(&buf).unwrap();
    file.flush().unwrap();
    file
}

/// Single program instance with a debug section — launches successfully.
fn single_instance_debug_container() -> NamedTempFile {
    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
        .max_call_depth(1)
        .add_var_name(a_var_name())
        .build();
    write_container(&container)
}

/// Single instance but no debug section — fails `NoDebugInfo`.
fn no_debug_container() -> NamedTempFile {
    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
        .max_call_depth(1)
        .build();
    write_container(&container)
}

/// Two program instances with a debug section — fails `MultiInstanceUnsupported`.
fn multi_instance_container() -> NamedTempFile {
    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
        .max_call_depth(1)
        .add_var_name(a_var_name())
        .add_task(a_task(TaskId::new(0)))
        .add_task(a_task(TaskId::new(1)))
        .add_program_instance(a_program(InstanceId::new(0), TaskId::new(0)))
        .add_program_instance(a_program(InstanceId::new(1), TaskId::new(1)))
        .build();
    write_container(&container)
}

/// Content-Length framing for a request value.
fn frame(request: &Value) -> Vec<u8> {
    let body = serde_json::to_vec(request).unwrap();
    let mut out = format!("Content-Length: {}\r\n\r\n", body.len()).into_bytes();
    out.extend_from_slice(&body);
    out
}

/// Decodes all Content-Length-framed messages in `bytes`.
fn parse_messages(bytes: &[u8]) -> Vec<Value> {
    let mut messages = Vec::new();
    let mut rest = bytes;
    while let Some(header_end) = find_subslice(rest, b"\r\n\r\n") {
        let header = std::str::from_utf8(&rest[..header_end]).unwrap();
        let len: usize = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length:"))
            .unwrap()
            .trim()
            .parse()
            .unwrap();
        let body_start = header_end + 4;
        let body_end = body_start + len;
        let body = &rest[body_start..body_end];
        messages.push(serde_json::from_slice(body).unwrap());
        rest = &rest[body_end..];
    }
    messages
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Spawns `ironplcvmd`, sends each request framed, closes stdin, and returns
/// the decoded response/event stream.
fn run_dap(requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(cargo::cargo_bin!("ironplcvmd"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();

    {
        let mut stdin = child.stdin.take().unwrap();
        for request in requests {
            stdin.write_all(&frame(request)).unwrap();
        }
        // stdin dropped here → the server sees EOF once it has drained input.
    }

    let mut stdout = child.stdout.take().unwrap();
    let mut out = Vec::new();
    stdout.read_to_end(&mut out).unwrap();
    child.wait().unwrap();

    parse_messages(&out)
}

#[test]
fn ironplcvmd_when_initialize_launch_disconnect_then_handshake_succeeds() {
    let container = single_instance_debug_container();
    let path = container.path().to_string_lossy().into_owned();

    let messages = run_dap(&[
        json!({"seq": 1, "type": "request", "command": "initialize",
               "arguments": {"adapterID": "ironplc"}}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"program": path}}),
        json!({"seq": 3, "type": "request", "command": "disconnect"}),
    ]);

    // The container's task is freewheeling, so the session also writes the
    // assumed-cycle-time notice to the debug console after the launch response.
    assert_eq!(messages.len(), 5, "messages: {messages:?}");

    // initialize response with the single advertised capability.
    assert_eq!(messages[0]["type"], "response");
    assert_eq!(messages[0]["command"], "initialize");
    assert_eq!(messages[0]["success"], true);
    assert_eq!(
        messages[0]["body"]["supportsConfigurationDoneRequest"],
        true
    );

    // initialized event follows.
    assert_eq!(messages[1]["type"], "event");
    assert_eq!(messages[1]["event"], "initialized");

    // launch response.
    assert_eq!(messages[2]["command"], "launch");
    assert_eq!(messages[2]["success"], true);
    assert_eq!(messages[2]["request_seq"], 2);

    // The assumed-cycle-time notice, naming the value the run used.
    assert_eq!(messages[3]["event"], "output");
    assert_eq!(messages[3]["body"]["category"], "console");
    assert!(messages[3]["body"]["output"]
        .as_str()
        .unwrap_or_default()
        .contains("100 ms"));

    // disconnect response.
    assert_eq!(messages[4]["command"], "disconnect");
    assert_eq!(messages[4]["success"], true);
}

#[test]
fn ironplcvmd_when_launch_container_without_debug_then_no_debug_info() {
    let container = no_debug_container();
    let path = container.path().to_string_lossy().into_owned();

    let messages = run_dap(&[
        json!({"seq": 1, "type": "request", "command": "initialize"}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"program": path}}),
    ]);

    let launch = messages.iter().find(|m| m["command"] == "launch").unwrap();
    assert_eq!(launch["success"], false);
    // The failure carries the V6009 launch-no-debug-info code.
    assert!(launch["message"].as_str().unwrap().starts_with("V6009 - "));
}

#[test]
fn ironplcvmd_when_launch_multi_instance_then_multi_instance_unsupported() {
    let container = multi_instance_container();
    let path = container.path().to_string_lossy().into_owned();

    let messages = run_dap(&[
        json!({"seq": 1, "type": "request", "command": "initialize"}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"program": path}}),
    ]);

    let launch = messages.iter().find(|m| m["command"] == "launch").unwrap();
    assert_eq!(launch["success"], false);
    // The failure carries the V6010 multi-instance code plus the descriptive text.
    let message = launch["message"].as_str().unwrap();
    assert!(message.starts_with("V6010 - "));
    assert!(message.contains("MultiInstanceUnsupported"));
}

// --- Timing: the debugger follows the task's INTERVAL (issue #1397). --------

/// The issue's reproduction: a 100 ms task driving a `TON` whose `PT` spans
/// five cycles. Under a flat 1 ms per scan the timer needed 500 scans.
const TON_ON_A_100MS_TASK: &str = "
PROGRAM plc_prg
  VAR
    timer : TON;
    q : BOOL;
  END_VAR
  timer(IN := TRUE, PT := T#500ms, Q => q);
END_PROGRAM

CONFIGURATION config
  RESOURCE res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM inst WITH plc_task : plc_prg;
  END_RESOURCE
END_CONFIGURATION
";

fn compile_to_container(source: &str) -> Container {
    let options = ironplc_parser::options::CompilerOptions::default();
    let library =
        ironplc_parser::parse_program(source, &ironplc_dsl::core::FileId::default(), &options)
            .unwrap();
    let (analyzed, context) =
        ironplc_analyzer::stages::resolve_types(&[&library], &options).unwrap();
    ironplc_codegen::compile(
        &analyzed,
        &context,
        &ironplc_codegen::CodegenOptions::default(),
        &ironplc_codegen::EmptyLookup,
    )
    .unwrap()
}

/// Walks the session one scan at a time, returning `(scanCount, systemUptime,
/// q)` at the start of each scan.
fn scan_by_scan(path: &str, scans: usize) -> Vec<(i64, i64, String)> {
    let mut requests = vec![
        json!({"seq": 1, "type": "request", "command": "initialize"}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"program": path, "stopOnEntry": true}}),
        json!({"seq": 3, "type": "request", "command": "configurationDone"}),
    ];
    let mut seq = 4;
    for _ in 0..scans {
        requests.push(
            json!({"seq": seq, "type": "request", "command": "variables",
                             "arguments": {"variablesReference": 2}}),
        );
        requests.push(
            json!({"seq": seq + 1, "type": "request", "command": "variables",
                             "arguments": {"variablesReference": 1}}),
        );
        requests.push(
            json!({"seq": seq + 2, "type": "request", "command": "ironplc/stepScan",
                             "arguments": {"threadId": 1}}),
        );
        seq += 3;
    }
    requests.push(json!({"seq": seq, "type": "request", "command": "disconnect"}));

    let messages = run_dap(&requests);
    let scopes: Vec<&Value> = messages
        .iter()
        .filter(|m| m["type"] == "response" && m["command"] == "variables")
        .collect();

    scopes
        .chunks(2)
        .filter(|pair| pair.len() == 2)
        .map(|pair| {
            let runtime = &pair[0]["body"]["variables"];
            let program = &pair[1]["body"]["variables"];
            let num = |v: &Value| -> i64 { v["value"].as_str().unwrap().parse().unwrap() };
            let q = program
                .as_array()
                .unwrap()
                .iter()
                .find(|v| v["name"] == "q")
                .map(|v| v["value"].as_str().unwrap().to_string())
                .unwrap_or_default();
            (num(&runtime[0]), num(&runtime[1]), q)
        })
        .collect()
}

#[test]
fn ironplcvmd_when_task_declares_interval_then_timers_follow_it() {
    // Issue #1397: program time advanced a flat 1 ms per scan, so this TON
    // elapsed at scan 500 rather than after the five 100 ms cycles its PT
    // actually spans.
    let container = write_container(&compile_to_container(TON_ON_A_100MS_TASK));
    let path = container.path().to_string_lossy().into_owned();

    let observed = scan_by_scan(&path, 8);

    let uptimes: Vec<i64> = observed.iter().map(|(_, uptime, _)| *uptime).collect();
    assert_eq!(
        uptimes,
        vec![0, 100, 200, 300, 400, 500, 600, 700],
        "each scan of a 100 ms task is 100 ms of program time"
    );

    let elapsed_at = observed
        .iter()
        .position(|(_, _, q)| q == "TRUE")
        .unwrap_or_else(|| panic!("Q never became TRUE within 8 scans: {observed:?}"));
    assert_eq!(
        observed[elapsed_at].0, 6,
        "PT := T#500ms spans five 100 ms cycles: {observed:?}"
    );
}
