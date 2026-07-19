//! DAP request legality per VM phase.
//!
//! The v1 server is a single-threaded state machine (see
//! `specs/plans/2026-06-25-dap-server-scaffold.md`). This module owns the
//! *legality table* only — a pure `legal(phase, command)` predicate. A request
//! that is illegal in the current phase short-circuits to a DAP error response
//! with the message `requestNotApplicable`, without touching the VM. The phase
//! *transitions* live with the server loop in a later commit (Phase 4.4); this
//! commit is data + predicate, with no VM.
//!
//! The predicate is consumed by that server loop, so for this commit it is
//! exercised only by the exhaustive unit tests below.
#![allow(dead_code)]

/// The VM lifecycle phase, mirrored on the DAP side so request legality can be
/// decided without reaching into the engine.
///
/// - [`Initialized`](Phase::Initialized): session opened; awaiting `initialize`.
/// - [`Configuring`](Phase::Configuring): initialized; the client is setting
///   breakpoints / launching before `configurationDone`.
/// - [`Running`](Phase::Running): the VM is executing under `run_round_debug`.
///   The single-threaded loop services no requests here — it reads the next
///   request only at a natural stop — so every request is illegal in this
///   phase except the always-legal `disconnect`.
/// - [`Paused`](Phase::Paused): stopped at a breakpoint, step landing, or entry
///   (non-terminal); inspection and execution control are accepted.
/// - [`Terminated`](Phase::Terminated): the program ran to completion; only
///   `disconnect` remains.
/// - [`Faulted`](Phase::Faulted): stopped on a trap (terminal pause). Inspection
///   is accepted so the failure can be examined; execution control is not.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    Initialized,
    Configuring,
    Running,
    Paused,
    Terminated,
    Faulted,
}

/// A DAP request the v1 server recognises.
///
/// This includes requests that are always refused ([`Pause`](Command::Pause),
/// [`SetVariable`](Command::SetVariable), [`Evaluate`](Command::Evaluate),
/// [`Restart`](Command::Restart)): modelling them explicitly lets the legality
/// table return the documented `requestNotApplicable` for a *known-but-cut*
/// request, distinct from an entirely unknown command.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Command {
    Initialize,
    Launch,
    SetBreakpoints,
    ConfigurationDone,
    Threads,
    StackTrace,
    Scopes,
    Variables,
    Continue,
    Next,
    StepIn,
    StepOut,
    Disconnect,
    // Known DAP requests deliberately unsupported in v1: always illegal.
    Pause,
    SetVariable,
    Evaluate,
    Restart,
}

impl Command {
    /// Map a DAP request `command` string to a [`Command`], or `None` for a
    /// command the v1 server does not model at all. Both `None` and a modelled
    /// but illegal command resolve to a `requestNotApplicable` response at the
    /// call site.
    pub fn from_request(command: &str) -> Option<Command> {
        let cmd = match command {
            "initialize" => Command::Initialize,
            "launch" => Command::Launch,
            "setBreakpoints" => Command::SetBreakpoints,
            "configurationDone" => Command::ConfigurationDone,
            "threads" => Command::Threads,
            "stackTrace" => Command::StackTrace,
            "scopes" => Command::Scopes,
            "variables" => Command::Variables,
            "continue" => Command::Continue,
            "next" => Command::Next,
            "stepIn" => Command::StepIn,
            "stepOut" => Command::StepOut,
            "disconnect" => Command::Disconnect,
            "pause" => Command::Pause,
            "setVariable" => Command::SetVariable,
            "evaluate" => Command::Evaluate,
            "restart" => Command::Restart,
            _ => return None,
        };
        Some(cmd)
    }
}

/// Whether `command` is accepted in `phase`. An illegal pair is answered with a
/// DAP `requestNotApplicable` error, never a VM action.
pub fn legal(phase: Phase, command: Command) -> bool {
    use Command::*;
    use Phase::*;
    match command {
        // Handshake.
        Initialize => phase == Initialized,
        Launch | ConfigurationDone => phase == Configuring,
        // Breakpoints can be (re)set before the run and at any live pause.
        SetBreakpoints => matches!(phase, Configuring | Paused),
        // Inspection: at any pause, including the terminal trap pause.
        Threads | StackTrace | Scopes | Variables => matches!(phase, Paused | Faulted),
        // Execution control: only at a non-terminal pause.
        Continue | Next | StepIn | StepOut => phase == Paused,
        // Teardown is always accepted.
        Disconnect => true,
        // Cut from v1: refused in every phase.
        Pause | SetVariable | Evaluate | Restart => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ALL_PHASES: [Phase; 6] = [
        Phase::Initialized,
        Phase::Configuring,
        Phase::Running,
        Phase::Paused,
        Phase::Terminated,
        Phase::Faulted,
    ];

    const ALL_COMMANDS: [Command; 17] = [
        Command::Initialize,
        Command::Launch,
        Command::SetBreakpoints,
        Command::ConfigurationDone,
        Command::Threads,
        Command::StackTrace,
        Command::Scopes,
        Command::Variables,
        Command::Continue,
        Command::Next,
        Command::StepIn,
        Command::StepOut,
        Command::Disconnect,
        Command::Pause,
        Command::SetVariable,
        Command::Evaluate,
        Command::Restart,
    ];

    /// The single source of truth the table is checked against: the exact set
    /// of phases each command is legal in. Any drift between this and `legal`
    /// fails the exhaustive test below.
    fn expected_legal_phases(command: Command) -> &'static [Phase] {
        use Command::*;
        use Phase::*;
        match command {
            Initialize => &[Initialized],
            Launch => &[Configuring],
            ConfigurationDone => &[Configuring],
            SetBreakpoints => &[Configuring, Paused],
            Threads | StackTrace | Scopes | Variables => &[Paused, Faulted],
            Continue | Next | StepIn | StepOut => &[Paused],
            Disconnect => &[
                Initialized,
                Configuring,
                Running,
                Paused,
                Terminated,
                Faulted,
            ],
            Pause | SetVariable | Evaluate | Restart => &[],
        }
    }

    #[test]
    fn legal_when_every_phase_command_pair_then_matches_expected_table() {
        for &command in &ALL_COMMANDS {
            let expected = expected_legal_phases(command);
            for &phase in &ALL_PHASES {
                let want = expected.contains(&phase);
                assert_eq!(
                    legal(phase, command),
                    want,
                    "legal({phase:?}, {command:?}) should be {want}"
                );
            }
        }
    }

    #[test]
    fn legal_when_command_is_cut_from_v1_then_illegal_in_every_phase() {
        for command in [
            Command::Pause,
            Command::SetVariable,
            Command::Evaluate,
            Command::Restart,
        ] {
            for &phase in &ALL_PHASES {
                assert!(
                    !legal(phase, command),
                    "{command:?} must be requestNotApplicable in {phase:?}"
                );
            }
        }
    }

    #[test]
    fn legal_when_disconnect_then_accepted_in_every_phase() {
        for &phase in &ALL_PHASES {
            assert!(legal(phase, Command::Disconnect));
        }
    }

    #[test]
    fn legal_when_running_then_only_disconnect_is_accepted() {
        // The single-threaded loop never services a request mid-run except the
        // teardown; the table encodes that invariant.
        for &command in &ALL_COMMANDS {
            let accepted = legal(Phase::Running, command);
            assert_eq!(accepted, command == Command::Disconnect);
        }
    }

    #[test]
    fn legal_when_faulted_then_inspection_yes_but_execution_control_no() {
        for command in [
            Command::Threads,
            Command::StackTrace,
            Command::Scopes,
            Command::Variables,
        ] {
            assert!(legal(Phase::Faulted, command));
        }
        for command in [
            Command::Continue,
            Command::Next,
            Command::StepIn,
            Command::StepOut,
        ] {
            assert!(!legal(Phase::Faulted, command));
        }
    }

    #[test]
    fn from_request_when_known_command_then_maps_to_variant() {
        assert_eq!(
            Command::from_request("configurationDone"),
            Some(Command::ConfigurationDone)
        );
        assert_eq!(Command::from_request("stepIn"), Some(Command::StepIn));
        // Known-but-cut requests still map, so the caller can answer
        // requestNotApplicable rather than "unknown command".
        assert_eq!(Command::from_request("pause"), Some(Command::Pause));
    }

    #[test]
    fn from_request_when_unknown_command_then_none() {
        assert_eq!(Command::from_request("ironplc/stepScan"), None);
        assert_eq!(Command::from_request(""), None);
    }
}
