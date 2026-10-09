fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "enumeration-codegen.md",
        "reference-to-twincat.md",
        // The codegen crate owns the execution requirements
        // (`REQ-PTR-codegen-*`) for the ADR operator and POINTER TO.
        "adr-and-pointer-to.md",
        // The function forms of operators (`REQ-KF-codegen-*`).
        "keyword-function-forms.md",
        // Arithmetic operator overloads (`REQ-AO-codegen-*`).
        "arithmetic-operator-overloads.md",
        // Comparison operand type (`REQ-CMP-codegen-*`).
        "comparison-operand-type.md",
        // Partial-access syntax (`REQ-PAB-codegen-*`): execution semantics.
        "partial-access-bit-syntax.md",
        // Behavior policies (`REQ-BP-codegen-*`): func_id selection.
        "behavior-policies.md",
        // Container format (`REQ-CF-codegen-*`): what the compiler writes into
        // the header. The container crate owns the rest of that doc.
        "bytecode-container-format.md",
        // Implicit conversions (`REQ-IC-codegen-*`): compiling the recorded
        // conversion of an argument.
        "implicit-conversions.md",
        // The post-emission peephole optimizer (`REQ-PEEP-codegen-*`).
        "bytecode-peephole-optimizer.md",
        // The resolved execution model (`REQ-EM-codegen-*`): the task table
        // and globals built from it, and the backend capability checks.
        "execution-model.md",
    ]);
}
