fn main() {
    // The sources crate owns the compatibility-library loader and the on-disk
    // package format, so it enforces both design docs' `sources`-slugged
    // requirements (`REQ-CL-sources-*` and `REQ-LF-sources-*`).
    ironplc_spec_requirements_gen::generate(&[
        "compatibility-libraries.md",
        "compatibility-library-format.md",
    ]);
}
