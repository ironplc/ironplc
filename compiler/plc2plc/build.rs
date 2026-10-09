fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "reference-to-twincat.md",
        // plc2plc owns the round-trip requirement (`REQ-CL-plc2plc-001`): user
        // source renders unchanged and injected library declarations are never
        // emitted.
        "compatibility-libraries.md",
        // plc2plc owns the POINTER TO rendering requirements
        // (`REQ-PTR-plc2plc-*`).
        "adr-and-pointer-to.md",
        // Partial-access syntax (`REQ-PAB-plc2plc-*`): round-trip rendering.
        "partial-access-bit-syntax.md",
        // Character string literals (`REQ-SL-plc2plc-*`): escaped rendering.
        "string-literals.md",
        // Inspecting the expression type annotation (`REQ-ETR-plc2plc-*`):
        // the annotated rendering.
        "expression-type-resolution.md",
        // Initial values (`REQ-IV-plc2plc-*`): an analyzed library renders
        // the initializers as written, without the values the analyzer
        // supplied.
        "initial-values.md",
    ]);
}
