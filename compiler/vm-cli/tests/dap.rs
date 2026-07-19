//! End-to-end handshake tests for the `ironplcdap` Debug Adapter Protocol
//! server (Phase 4.3). These spawn the real binary and drive the
//! `initialize` → `launch` → `disconnect` path over stdin/stdout, asserting
//! the framed responses and events, plus the two launch-precondition failures
//! (`NoDebugInfo`, `MultiInstanceUnsupported`).
//!
//! Gated on the `dap` feature: the `ironplcdap` binary only exists when the
//! feature is enabled (CI builds it via `--features ironplc-vm-cli/dap`).
#![cfg(feature = "dap")]

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
        .add_var_name(a_var_name())
        .build();
    write_container(&container)
}

/// Single instance but no debug section — fails `NoDebugInfo`.
fn no_debug_container() -> NamedTempFile {
    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
        .build();
    write_container(&container)
}

/// Two program instances with a debug section — fails `MultiInstanceUnsupported`.
fn multi_instance_container() -> NamedTempFile {
    let container = ContainerBuilder::new()
        .num_variables(1)
        .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
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
        let header = std::str::from_utf8(&rest[..header_end]).expect("ascii header");
        let len: usize = header
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length:"))
            .expect("Content-Length header")
            .trim()
            .parse()
            .expect("numeric Content-Length");
        let body_start = header_end + 4;
        let body_end = body_start + len;
        let body = &rest[body_start..body_end];
        messages.push(serde_json::from_slice(body).expect("json body"));
        rest = &rest[body_end..];
    }
    messages
}

fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Spawns `ironplcdap`, sends each request framed, closes stdin, and returns
/// the decoded response/event stream.
fn run_dap(requests: &[Value]) -> Vec<Value> {
    let mut child = Command::new(cargo::cargo_bin!("ironplcdap"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn ironplcdap");

    {
        let mut stdin = child.stdin.take().expect("child stdin");
        for request in requests {
            stdin.write_all(&frame(request)).expect("write request");
        }
        // stdin dropped here → the server sees EOF once it has drained input.
    }

    let mut stdout = child.stdout.take().expect("child stdout");
    let mut out = Vec::new();
    stdout.read_to_end(&mut out).expect("read stdout");
    child.wait().expect("wait for ironplcdap");

    parse_messages(&out)
}

#[test]
fn ironplcdap_when_initialize_launch_disconnect_then_handshake_succeeds() {
    let container = single_instance_debug_container();
    let path = container.path().to_string_lossy().into_owned();

    let messages = run_dap(&[
        json!({"seq": 1, "type": "request", "command": "initialize",
               "arguments": {"adapterID": "ironplc"}}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"program": path}}),
        json!({"seq": 3, "type": "request", "command": "disconnect"}),
    ]);

    assert_eq!(messages.len(), 4, "messages: {messages:?}");

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

    // disconnect response.
    assert_eq!(messages[3]["command"], "disconnect");
    assert_eq!(messages[3]["success"], true);
}

#[test]
fn ironplcdap_when_launch_container_without_debug_then_no_debug_info() {
    let container = no_debug_container();
    let path = container.path().to_string_lossy().into_owned();

    let messages = run_dap(&[
        json!({"seq": 1, "type": "request", "command": "initialize"}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"program": path}}),
    ]);

    let launch = messages
        .iter()
        .find(|m| m["command"] == "launch")
        .expect("launch response");
    assert_eq!(launch["success"], false);
    assert!(launch["message"].as_str().unwrap().contains("NoDebugInfo"));
}

#[test]
fn ironplcdap_when_launch_multi_instance_then_multi_instance_unsupported() {
    let container = multi_instance_container();
    let path = container.path().to_string_lossy().into_owned();

    let messages = run_dap(&[
        json!({"seq": 1, "type": "request", "command": "initialize"}),
        json!({"seq": 2, "type": "request", "command": "launch",
               "arguments": {"program": path}}),
    ]);

    let launch = messages
        .iter()
        .find(|m| m["command"] == "launch")
        .expect("launch response");
    assert_eq!(launch["success"], false);
    assert!(launch["message"]
        .as_str()
        .unwrap()
        .contains("MultiInstanceUnsupported"));
}
