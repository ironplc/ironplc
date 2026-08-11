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
    ]);
}
