//! Spec conformance tests for the execution model's types.
//!
//! The tests check the requirements this crate owns in
//! `specs/design/execution-model.md`, and each names its requirement in a
//! comment. They become `#[spec_test(REQ_EM_ir_NNN)]` when `build.rs` lists
//! that document (see the comment there); until then the
//! `all_spec_requirements_have_tests` meta-test has no requirement to check.

use ironplc_dsl::core::SourceSpan;

use crate::execution::{
    Configuration, DebugName, Execution, ExecutionModelBuilder, Global, GlobalKind, GlobalScope,
    NotExecutable, ProgramInstance, ProgramType, Resource, Schedule, Task, Trigger,
};

// ---------------------------------------------------------------------------
// Meta-test: completeness check
// ---------------------------------------------------------------------------

#[test]
fn all_spec_requirements_have_tests() {
    // UNTESTED is computed by build.rs by scanning all .rs files under src/
    // for #[spec_test(REQ_...)] attributes.
    assert!(
        crate::spec_requirements::UNTESTED.is_empty(),
        "Requirements in spec with no conformance test: {:?}",
        crate::spec_requirements::UNTESTED
    );
}

// ---------------------------------------------------------------------------
// What the model holds (REQ-EM-ir-001, REQ-EM-ir-002)
// ---------------------------------------------------------------------------

/// Asks whether a concrete type implements a trait, as a `bool`.
///
/// A missing trait cannot be asserted with a bound, so this asks method
/// resolution instead ("autoref specialization"). For `Probe<T>`, the method
/// of `$implements` is reached without an autoref, and applies only when `T`
/// implements `$trait`; the method of `$lacks` is on `&Probe<T>`, one autoref
/// further, so it answers only when the first does not apply. Call the method
/// on `&Probe::<T>::new()` with a concrete `T`: in a generic function the
/// answer is always `false`.
macro_rules! trait_probe {
    ($implements:ident, $lacks:ident, $method:ident, $trait:path) => {
        trait $implements {
            fn $method(&self) -> bool {
                true
            }
        }
        impl<T: $trait> $implements for Probe<T> {}
        trait $lacks {
            fn $method(&self) -> bool {
                false
            }
        }
        impl<T> $lacks for &Probe<T> {}
    };
}

struct Probe<T>(std::marker::PhantomData<T>);

impl<T> Probe<T> {
    fn new() -> Self {
        Self(std::marker::PhantomData)
    }
}

trait_probe!(ImplementsPartialEq, LacksPartialEq, partial_eq, PartialEq);
trait_probe!(ImplementsEq, LacksEq, eq, Eq);
trait_probe!(ImplementsHash, LacksHash, hash, std::hash::Hash);
trait_probe!(ImplementsOrd, LacksOrd, ord, Ord);

#[test]
fn trait_probe_when_type_implements_trait_then_true() {
    let probe = &Probe::<u32>::new();

    assert!(probe.partial_eq());
    assert!(probe.eq());
    assert!(probe.hash());
    assert!(probe.ord());
}

#[test]
fn trait_probe_when_type_lacks_trait_then_false() {
    struct Bare;
    let probe = &Probe::<Bare>::new();

    assert!(!probe.partial_eq());
    assert!(!probe.eq());
    assert!(!probe.hash());
    assert!(!probe.ord());
}

// REQ-EM-ir-001
#[test]
fn debug_name_when_probed_then_implements_no_comparison_or_hash() {
    let probe = &Probe::<DebugName>::new();

    assert!(!probe.partial_eq());
    assert!(!probe.eq());
    assert!(!probe.hash());
    assert!(!probe.ord());
}

/// The `ironplc_dsl` items the module of `source` names.
fn dsl_items(source: &str) -> Vec<&str> {
    source
        .split("ironplc_dsl::")
        .skip(1)
        .filter_map(|after| after.split(|c: char| c == ';' || c.is_whitespace()).next())
        .collect()
}

// REQ-EM-ir-001
#[test]
fn execution_module_when_scanned_then_names_only_source_span_from_dsl() {
    // A declaration node, an `Id` or a `TypeId` would have to be named
    // through `ironplc_dsl`; the only item the module names is the span a
    // `DebugName` keeps.
    let items = dsl_items(include_str!("execution.rs"));

    assert!(!items.is_empty());
    assert!(items.iter().all(|item| *item == "core::SourceSpan"));
}

/// Asserts at compile time that the type of `value` borrows nothing, such as
/// a library.
fn holds_no_reference<T: 'static>(_value: &T) -> bool {
    true
}

// REQ-EM-ir-001
#[test]
fn execution_when_built_then_borrows_nothing_from_a_library() {
    let execution = Execution::NotExecutable(NotExecutable::NoProgram);

    assert!(holds_no_reference(&execution));
}

// REQ-EM-ir-001
#[test]
fn execution_model_when_built_then_every_id_names_an_entry_of_the_model() {
    let name = |text: &str| DebugName::new(text, SourceSpan::range(0, text.len()));
    let mut builder = ExecutionModelBuilder::new();
    let program = builder.add_program(ProgramType { name: name("main") });
    let trigger = builder.add_global(Global {
        name: name("start"),
        kind: GlobalKind::Declared(GlobalScope::Configuration),
    });
    let model = builder.build(Configuration {
        name: Some(name("config")),
        resources: vec![Resource {
            name: Some(name("resource1")),
            tasks: vec![Task {
                name: Some(name("on_start")),
                priority: 1,
                schedule: Schedule::Event {
                    trigger: Trigger::Global(trigger),
                    interval: None,
                },
                instances: vec![ProgramInstance {
                    name: Some(name("instance1")),
                    program,
                }],
            }],
        }],
    });

    let task = &model.configuration().resources[0].tasks[0];
    assert_eq!(
        model
            .program(task.instances[0].program)
            .map(|p| p.name.to_string()),
        Some("main".to_string())
    );
    assert!(matches!(
        task.schedule,
        Schedule::Event { trigger: Trigger::Global(id), .. }
            if model.global(id).map(|g| g.name.to_string()) == Some("start".to_string())
    ));
}

/// The names of the `[dependencies]` of `manifest` given by path: the crates
/// of this workspace.
fn workspace_dependencies(manifest: &str) -> Vec<&str> {
    manifest
        .split("\n[")
        .filter(|section| section.starts_with("dependencies]"))
        .flat_map(|section| section.lines().skip(1))
        .filter(|line| !line.trim_start().starts_with('#') && line.contains("path ="))
        .filter_map(|line| line.split_once('=').map(|(name, _)| name.trim()))
        .collect()
}

// REQ-EM-ir-002
#[test]
fn ir_manifest_when_read_then_depends_on_no_compiler_crate_but_dsl() {
    // Build and dev dependencies do not reach a crate that depends on
    // `ironplc-ir`, so only `[dependencies]` counts.
    assert_eq!(
        workspace_dependencies(include_str!("../Cargo.toml")),
        vec!["ironplc-dsl"]
    );
}
