fn main() {
    // The playground owns the browser-activation requirement
    // (`REQ-CL-playground-001`) for compatibility libraries.
    ironplc_spec_requirements_gen::generate(&["compatibility-libraries.md"]);
}
