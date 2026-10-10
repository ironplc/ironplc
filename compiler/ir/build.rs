fn main() {
    // This crate owns the REQ-EM-ir-* requirements of
    // specs/design/execution-model.md, and its tests name them. The document
    // is listed here, and the tests become `#[spec_test]`, in the same change
    // that has the analyzer and codegen list it: the workspace guard
    // `every_requirement_slug_is_claimed_by_a_listing_crate` requires every
    // crate a listed document names to list it too.
    ironplc_spec_requirements_gen::generate(&[]);
}
