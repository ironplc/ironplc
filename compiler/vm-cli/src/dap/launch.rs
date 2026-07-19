//! `launch` preconditions and VM construction.
//!
//! On a DAP `launch`, the server loads the requested container, checks the two
//! v1 preconditions, and — if they hold — sizes the VM buffers and starts the
//! VM. The buffer sizing reuses `VmBuffers::from_container` (the same embedding
//! path the production `ironplcvm` binary uses in `cli.rs`), so there is no
//! duplicated sizing logic here.
//!
//! The preconditions (see the plan,
//! `specs/plans/2026-06-25-dap-server-scaffold.md` §"Launch preconditions"):
//! 1. A debug section must be present, else [`LaunchError::NoDebugInfo`].
//! 2. There must be exactly one program instance, else
//!    [`LaunchError::MultiInstanceUnsupported`] (the v1 limitation described in
//!    `specs/design/debugger-support.md` §"Multi-instance: not supported in v1").

use std::fmt;
use std::fs::File;
use std::path::Path;

use ironplc_container::Container;
use ironplc_vm::{Vm, VmBuffers, VmRunning};

use super::problem_codes;

/// A reason a `launch` request could not be satisfied.
///
/// Each variant carries a stable IronPLC [`v_code`](LaunchError::v_code); the
/// [`Display`] rendering is `"V#### - message"`, matching the CLI's `VmError`
/// surface, and is what fills the failing DAP response's `message` field.
#[derive(Debug)]
pub enum LaunchError {
    /// The `launch` arguments carried no usable `program` path.
    ProgramArgMissing,
    /// The container file could not be opened.
    ContainerOpen(String),
    /// The container file could not be parsed.
    ContainerRead(String),
    /// The container has no debug section, so no source-level debugging is
    /// possible.
    NoDebugInfo,
    /// The container declares more than one program instance; v1 debugs
    /// single-instance programs only. Carries the declared instance count.
    MultiInstanceUnsupported(usize),
    /// The VM could not be started (an init function trapped). Carries the
    /// trap's own V-code and its description.
    VmStartFailed {
        v_code: &'static str,
        detail: String,
    },
}

impl LaunchError {
    /// The stable V-code for this failure. File errors reuse the CLI's existing
    /// `V6001`/`V6002`; a start-time trap surfaces the trap's own `V4xxx`/
    /// `V9xxx`; the launch-specific preconditions use the `V6008`–`V6010` codes.
    pub fn v_code(&self) -> &'static str {
        match self {
            LaunchError::ProgramArgMissing => problem_codes::LAUNCH_NO_PROGRAM,
            LaunchError::ContainerOpen(_) => problem_codes::FILE_OPEN,
            LaunchError::ContainerRead(_) => problem_codes::CONTAINER_READ,
            LaunchError::NoDebugInfo => problem_codes::LAUNCH_NO_DEBUG_INFO,
            LaunchError::MultiInstanceUnsupported(_) => problem_codes::LAUNCH_MULTI_INSTANCE,
            LaunchError::VmStartFailed { v_code, .. } => v_code,
        }
    }

    /// The human-readable text (without the V-code prefix). The
    /// spec-mandated `MultiInstanceUnsupported:` wording is preserved verbatim
    /// (see `specs/design/debugger-support.md` §"Multi-instance").
    pub fn message(&self) -> String {
        match self {
            LaunchError::ProgramArgMissing => {
                "launch requires a 'program' path to a compiled .iplc container".to_string()
            }
            LaunchError::ContainerOpen(detail) => format!("unable to open container: {detail}"),
            LaunchError::ContainerRead(detail) => format!("unable to read container: {detail}"),
            LaunchError::NoDebugInfo => "compile with debug info enabled".to_string(),
            LaunchError::MultiInstanceUnsupported(count) => format!(
                "MultiInstanceUnsupported: this program declares {count} program instances; \
                 the v1 debugger supports single-instance programs only. Multi-instance \
                 debugging is planned for a future phase."
            ),
            LaunchError::VmStartFailed { detail, .. } => {
                format!("launch failed to start the VM: {detail}")
            }
        }
    }
}

impl fmt::Display for LaunchError {
    /// Renders `"V#### - message"`, matching the CLI's `VmError` surface so a
    /// DAP client sees the same coded error text.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} - {}", self.v_code(), self.message())
    }
}

/// Opens and parses the container at `path`.
pub fn load_container(path: &Path) -> Result<Container, LaunchError> {
    let mut file = File::open(path)
        .map_err(|e| LaunchError::ContainerOpen(format!("{}: {e}", path.display())))?;
    Container::read_from(&mut file)
        .map_err(|e| LaunchError::ContainerRead(format!("{}: {e}", path.display())))
}

/// Checks the two v1 launch preconditions against a loaded container.
///
/// Debug info is checked first (per the plan), then the single-instance limit,
/// so a container that is both missing debug info and multi-instance reports
/// [`LaunchError::NoDebugInfo`].
pub fn check_preconditions(container: &Container) -> Result<(), LaunchError> {
    if container.debug_section.is_none() {
        return Err(LaunchError::NoDebugInfo);
    }
    let instances = container.task_table.programs.len();
    if instances != 1 {
        return Err(LaunchError::MultiInstanceUnsupported(instances));
    }
    Ok(())
}

/// Loads the container, starts the VM into the caller-owned `bufs`, and returns
/// the running VM.
///
/// The caller sizes `bufs` with [`VmBuffers::from_container`] and owns both
/// `container` and `bufs` so the returned [`VmRunning`] can borrow them. This
/// mirrors the `ironplcvm` `Run` embedding in `cli.rs`; the only added policy
/// is mapping a start-time trap to [`LaunchError::VmStartFailed`].
pub fn start_vm<'a>(
    container: &'a Container,
    bufs: &'a mut VmBuffers,
) -> Result<VmRunning<'a>, LaunchError> {
    Vm::new()
        .load(container, bufs)
        .start()
        .map_err(|ctx| LaunchError::VmStartFailed {
            v_code: ctx.trap.v_code(),
            detail: ctx.trap.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use ironplc_container::debug_section::{iec_type_tag, var_section, VarNameEntry};
    use ironplc_container::{
        ContainerBuilder, FunctionId, InstanceId, ProgramInstanceEntry, TaskEntry, TaskId,
        TaskType, VarIndex,
    };

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

    #[test]
    fn check_preconditions_when_debug_and_single_instance_then_ok() {
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .add_var_name(a_var_name())
            .build();
        assert!(check_preconditions(&container).is_ok());
    }

    #[test]
    fn check_preconditions_when_no_debug_section_then_no_debug_info() {
        // No debug entries → builder emits no debug section.
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .build();
        let err = check_preconditions(&container).unwrap_err();
        assert!(matches!(err, LaunchError::NoDebugInfo));
        assert_eq!(err.v_code(), "V6009");
        assert!(err.message().contains("debug info"));
        // Display prefixes the V-code.
        assert!(err.to_string().starts_with("V6009 - "));
    }

    #[test]
    fn check_preconditions_when_multiple_instances_then_multi_instance_unsupported() {
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .add_var_name(a_var_name())
            .add_task(a_task(TaskId::new(0)))
            .add_task(a_task(TaskId::new(1)))
            .add_program_instance(a_program(InstanceId::new(0), TaskId::new(0)))
            .add_program_instance(a_program(InstanceId::new(1), TaskId::new(1)))
            .build();
        let err = check_preconditions(&container).unwrap_err();
        assert!(matches!(err, LaunchError::MultiInstanceUnsupported(2)));
        assert_eq!(err.v_code(), "V6010");
        assert!(err.message().contains("MultiInstanceUnsupported"));
        assert!(err.message().contains("2 program instances"));
        assert!(err.to_string().starts_with("V6010 - "));
    }

    #[test]
    fn start_vm_when_single_instance_debug_container_then_runs() {
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_function(FunctionId::new(0), &[0x8C], 0, 1, 0)
            .add_var_name(a_var_name())
            .build();
        let mut bufs = VmBuffers::from_container(&container);
        assert!(start_vm(&container, &mut bufs).is_ok());
    }

    #[test]
    fn start_vm_when_init_traps_then_vm_start_failed() {
        // The (default) init function divides by zero, so `start()` traps.
        #[rustfmt::skip]
        let bytecode: Vec<u8> = vec![
            0x00, 0x00, 0x00, // LOAD_CONST_I32 pool[0] (10)
            0x00, 0x01, 0x00, // LOAD_CONST_I32 pool[1] (0)
            0x30,             // DIV_I32 -> DivideByZero
            0x8C,             // RET_VOID
        ];
        let container = ContainerBuilder::new()
            .num_variables(1)
            .add_i32_constant(10)
            .add_i32_constant(0)
            .add_function(FunctionId::new(0), &bytecode, 2, 1, 0)
            .add_var_name(a_var_name())
            .build();
        let mut bufs = VmBuffers::from_container(&container);
        let err = match start_vm(&container, &mut bufs) {
            Ok(_) => panic!("expected the dividing-by-zero init to trap"),
            Err(err) => err,
        };
        assert!(matches!(err, LaunchError::VmStartFailed { .. }));
        // The start-time trap surfaces its own V-code (divide by zero → V4001).
        assert_eq!(err.v_code(), "V4001");
        assert!(err.message().contains("launch failed to start"));
        assert!(err.to_string().starts_with("V4001 - "));
    }

    #[test]
    fn load_container_when_missing_file_then_container_open_error() {
        let err = load_container(Path::new("does/not/exist.iplc")).unwrap_err();
        assert!(matches!(err, LaunchError::ContainerOpen(_)));
        assert!(err.message().contains("unable to open"));
        // Reuses the CLI's existing file-open code.
        assert_eq!(err.v_code(), "V6001");
    }

    #[test]
    fn load_container_when_file_is_not_a_container_then_container_read_error() {
        use std::io::Write as _;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(b"this is not a container").unwrap();
        file.flush().unwrap();
        let err = load_container(file.path()).unwrap_err();
        assert!(matches!(err, LaunchError::ContainerRead(_)));
        assert!(err.message().contains("unable to read container"));
        // Reuses the CLI's existing container-read code.
        assert_eq!(err.v_code(), "V6002");
    }

    #[test]
    fn message_when_program_arg_missing_then_mentions_program_path() {
        assert!(LaunchError::ProgramArgMissing
            .message()
            .contains("'program' path"));
        assert_eq!(LaunchError::ProgramArgMissing.v_code(), "V6008");
        assert_eq!(
            LaunchError::ProgramArgMissing.to_string(),
            "V6008 - launch requires a 'program' path to a compiled .iplc container"
        );
    }
}
