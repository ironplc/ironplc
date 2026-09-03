fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "enumeration-codegen.md",
        "reference-to-twincat.md",
        // The codegen crate owns the execution requirements
        // (`REQ-PTR-codegen-*`) for the ADR operator and POINTER TO.
        "adr-and-pointer-to.md",
        // The function forms of operators (`REQ-KF-codegen-*`).
        "keyword-function-forms.md",
    ]);
}
