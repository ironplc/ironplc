fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "reference-to-twincat.md",
        // The analyzer owns the resolution/scoping requirements
        // (`REQ-CL-analyzer-*`) for activated compatibility libraries.
        "compatibility-libraries.md",
        // The analyzer owns the explicit-dereference semantics requirements
        // (`REQ-PTR-analyzer-*`) for POINTER TO.
        "adr-and-pointer-to.md",
        // The function forms of operators (`REQ-KF-analyzer-*`).
        "keyword-function-forms.md",
        // Arithmetic operator overloads (`REQ-AO-analyzer-*`).
        "arithmetic-operator-overloads.md",
        // Implicit conversions recorded in the AST (`REQ-IC-analyzer-*`).
        "implicit-conversions.md",
        // Comparison operand type (`REQ-CMP-analyzer-*`).
        "comparison-operand-type.md",
        // Partial-access syntax (`REQ-PAB-analyzer-*`): slice range checks.
        "partial-access-bit-syntax.md",
        // Constant variable inference (`REQ-CVI-analyzer-*`): which
        // never-written declarations become CONSTANT.
        "constant-variable-inference.md",
        // Inspecting the expression type annotation (`REQ-ETR-analyzer-*`):
        // how a type is spelled in the annotated rendering.
        "expression-type-resolution.md",
        // The resolved execution model (`REQ-EM-analyzer-*`): configuration,
        // program instances, tasks and globals.
        "execution-model.md",
    ]);
}
