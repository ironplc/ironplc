fn main() {
    ironplc_spec_requirements_gen::generate(&[
        // The execution model's types (`REQ-EM-ir-*`). The analyzer and
        // codegen own the rest of the document.
        "execution-model.md",
    ]);
}
