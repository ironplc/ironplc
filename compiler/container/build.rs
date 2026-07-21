fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "bytecode-container-format.md",
        "bytecode-instruction-set.md",
        // The container owns the ENUM_DEF payload requirement (REQ-EN-container-*)
        // from the enumeration design; codegen owns the rest of that same doc.
        // This is the cross-crate conformance case: one design doc, two crates.
        "enumeration-codegen.md",
    ]);
}
