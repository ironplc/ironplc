fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "enumeration-codegen.md",
        "reference-to-twincat.md",
        // Codegen owns the compatibility-library binding requirements
        // (`REQ-CL-codegen-*`): intrinsic-bound call lowering and the
        // declare-only fail-if-unimplemented rule.
        "compatibility-libraries.md",
    ]);
}
