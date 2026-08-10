//! Compatibility-library manifest (`library.toml`) parsing and validation.
//!
//! A manifest declares a compatibility library's identity and records the
//! public references it was authored from. The on-disk format is specified in
//! `specs/design/compatibility-library-format.md` (`REQ-LF-sources-*`) and the
//! behavioral requirements the loader enforces are in
//! `specs/design/compatibility-libraries.md` (`REQ-CL-sources-002`).

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

/// A parsed and validated `library.toml` manifest.
///
/// Every field is required except `implicit`. `vendor` is nominative — it
/// records whose interface the library mirrors, not a claim of endorsement
/// (see the format design's *Non-affiliation* section).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LibraryManifest {
    /// Library identity; equals the directory name and the vendor library name.
    pub name: String,
    /// Whose interface the library mirrors (e.g. `Beckhoff Automation GmbH`).
    pub vendor: String,
    /// The version used when a reference does not pin one; names one of the
    /// version subdirectories.
    pub default_version: String,
    /// The public references the library was authored from. Non-empty.
    pub references: Vec<String>,
    /// Whether the vendor environment provides this library to every project
    /// without a reference (`REQ-LF-sources-008`). Implicit bundled libraries
    /// activate automatically when a TwinCAT project is discovered.
    pub implicit: bool,
}

impl LibraryManifest {
    /// Parse and validate a manifest from `library.toml` text.
    ///
    /// The loader rejects a manifest that is malformed or is missing any
    /// required field (`REQ-CL-sources-002`, `REQ-LF-sources-002`), including
    /// an empty `references` list (`REQ-LF-sources-004`). `file_id` locates the
    /// diagnostic on the offending manifest.
    pub fn from_toml(content: &str, file_id: &FileId) -> Result<Self, Diagnostic> {
        let table: toml::Table = toml::from_str(content)
            .map_err(|e| Self::invalid(file_id, format!("manifest is not valid TOML: {e}")))?;

        let name = Self::required_string(&table, "name", file_id)?;
        let vendor = Self::required_string(&table, "vendor", file_id)?;
        let default_version = Self::required_string(&table, "default_version", file_id)?;

        let references_value = table.get("references").ok_or_else(|| {
            Self::invalid(
                file_id,
                "manifest is missing the required `references` field",
            )
        })?;
        let references_array = references_value.as_array().ok_or_else(|| {
            Self::invalid(
                file_id,
                "manifest field `references` must be an array of strings",
            )
        })?;
        if references_array.is_empty() {
            return Err(Self::invalid(
                file_id,
                "manifest field `references` must not be empty",
            ));
        }
        let mut references = Vec::with_capacity(references_array.len());
        for entry in references_array {
            let reference = entry.as_str().ok_or_else(|| {
                Self::invalid(
                    file_id,
                    "manifest field `references` must be an array of strings",
                )
            })?;
            references.push(reference.to_string());
        }

        let implicit = match table.get("implicit") {
            None => false,
            Some(value) => value.as_bool().ok_or_else(|| {
                Self::invalid(file_id, "manifest field `implicit` must be a boolean")
            })?,
        };

        Ok(LibraryManifest {
            name,
            vendor,
            default_version,
            references,
            implicit,
        })
    }

    /// Read a required, non-empty string field from the manifest table.
    fn required_string(
        table: &toml::Table,
        field: &str,
        file_id: &FileId,
    ) -> Result<String, Diagnostic> {
        let field_value = table.get(field).ok_or_else(|| {
            Self::invalid(
                file_id,
                format!("manifest is missing the required `{field}` field"),
            )
        })?;
        let text = field_value.as_str().ok_or_else(|| {
            Self::invalid(
                file_id,
                format!("manifest field `{field}` must be a string"),
            )
        })?;
        if text.is_empty() {
            return Err(Self::invalid(
                file_id,
                format!("manifest field `{field}` must not be empty"),
            ));
        }
        Ok(text.to_string())
    }

    /// Build a `LibraryManifestInvalid` (P6010) diagnostic located on the
    /// manifest file.
    fn invalid(file_id: &FileId, message: impl Into<String>) -> Diagnostic {
        Diagnostic::problem(
            Problem::LibraryManifestInvalid,
            Label::file(file_id.clone(), message),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file_id() -> FileId {
        FileId::from_string("library.toml")
    }

    #[test]
    fn from_toml_when_all_fields_present_then_parses() {
        let content = r#"
name = "Tc2_System"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
references = [ "https://example.com/reference" ]
"#;
        let manifest = LibraryManifest::from_toml(content, &file_id()).unwrap();
        assert_eq!(manifest.name, "Tc2_System");
        assert_eq!(manifest.vendor, "Beckhoff Automation GmbH");
        assert_eq!(manifest.default_version, "1.0.0");
        assert_eq!(manifest.references, vec!["https://example.com/reference"]);
        assert!(!manifest.implicit);
    }

    #[test]
    fn from_toml_when_implicit_true_then_parses() {
        let content = r#"
name = "Tc2_BuiltIns"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
implicit = true
references = [ "https://example.com/reference" ]
"#;
        let manifest = LibraryManifest::from_toml(content, &file_id()).unwrap();
        assert!(manifest.implicit);
    }

    #[test]
    fn from_toml_when_implicit_not_boolean_then_error() {
        let content = r#"
name = "Tc2_BuiltIns"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
implicit = "yes"
references = [ "https://example.com/reference" ]
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_missing_name_then_error() {
        let content = r#"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
references = [ "https://example.com/reference" ]
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_missing_vendor_then_error() {
        let content = r#"
name = "Tc2_System"
default_version = "1.0.0"
references = [ "https://example.com/reference" ]
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_missing_default_version_then_error() {
        let content = r#"
name = "Tc2_System"
vendor = "Beckhoff Automation GmbH"
references = [ "https://example.com/reference" ]
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_missing_references_then_error() {
        let content = r#"
name = "Tc2_System"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_references_empty_then_error() {
        let content = r#"
name = "Tc2_System"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
references = []
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_field_wrong_type_then_error() {
        let content = r#"
name = 42
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
references = [ "https://example.com/reference" ]
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_references_not_strings_then_error() {
        let content = r#"
name = "Tc2_System"
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
references = [ 1, 2, 3 ]
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_malformed_toml_then_error() {
        let content = "this is not = valid toml {{{";
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_empty_name_then_error() {
        let content = r#"
name = ""
vendor = "Beckhoff Automation GmbH"
default_version = "1.0.0"
references = [ "https://example.com/reference" ]
"#;
        let err = LibraryManifest::from_toml(content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }
}
