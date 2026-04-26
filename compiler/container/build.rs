fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "bytecode-container-format.md",
        "bytecode-instruction-set.md",
    ]);
}
