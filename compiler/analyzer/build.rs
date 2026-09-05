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
        // Partial-access syntax (`REQ-PAB-analyzer-*`): slice range checks.
        "partial-access-bit-syntax.md",
    ]);
}
