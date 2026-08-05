fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "reference-to-twincat.md",
        // The analyzer owns the resolution/scoping requirements
        // (`REQ-CL-analyzer-*`) for activated compatibility libraries.
        "compatibility-libraries.md",
    ]);
}
