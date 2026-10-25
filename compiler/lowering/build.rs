fn main() {
    ironplc_spec_requirements_gen::generate(&[
        // The resolved execution model (`REQ-EM-lowering-*`): the
        // configuration, its tasks and program instances, and the globals in
        // scope. The analyzer, the IR and codegen own the rest of the
        // document.
        "execution-model.md",
    ]);
}
