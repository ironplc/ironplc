fn main() {
    ironplc_spec_requirements_gen::generate(&[
        "reference-to-twincat.md",
        "adr-and-pointer-to.md",
        // Partial-access syntax (`REQ-PAB-parser-*`): tokens, grammar, AST and
        // gating.
        "partial-access-bit-syntax.md",
    ]);
}
