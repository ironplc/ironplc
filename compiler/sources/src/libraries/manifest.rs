//! Compatibility-library manifest (`library.toml`) parsing and validation.
//!
//! A manifest declares a compatibility library's identity and records the
//! public references it was authored from. The on-disk format is specified in
//! `specs/design/compatibility-library-format.md` (`REQ-LF-sources-*`) and the
//! behavioral requirements the loader enforces are in
//! `specs/design/compatibility-libraries.md` (`REQ-CL-sources-002`).

use std::collections::HashMap;

use ironplc_dsl::core::FileId;
use ironplc_dsl::diagnostic::{Diagnostic, Label};
use ironplc_problems::Problem;

/// A parsed and validated `library.toml` manifest.
///
/// Every field is required except the per-version bindings tables. `vendor`
/// is nominative — it records whose interface the library mirrors, not a
/// claim of endorsement (see the format design's *Non-affiliation* section).
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
    /// Per-version declare-only bindings: version → uppercased POU names
    /// whose implementation has not been built, so a call is a compile
    /// error. Versions without a bindings table are absent.
    pub declare_only: HashMap<String, Vec<String>>,
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

        let declare_only = Self::parse_version_bindings(&table, file_id)?;

        Ok(LibraryManifest {
            name,
            vendor,
            default_version,
            references,
            declare_only,
        })
    }

    /// Parse and shape-validate every per-version bindings table
    /// (`REQ-LF-sources-005`, `REQ-LF-sources-006`).
    ///
    /// A version table is any top-level key whose value is a table (the
    /// scalar identity/reference fields are handled above). Validation covers
    /// *every* version table, not only the `default_version`, so a malformed
    /// table cannot hide in an inactive version. The unquoted-key trap —
    /// `[1.0.0.bindings]` parses as three nested tables `1 → 0 → 0` — is
    /// caught because the nested tables carry keys other than `bindings`.
    fn parse_version_bindings(
        table: &toml::Table,
        file_id: &FileId,
    ) -> Result<HashMap<String, Vec<String>>, Diagnostic> {
        const SCALAR_FIELDS: [&str; 4] = ["name", "vendor", "default_version", "references"];

        let mut all_bindings = HashMap::new();
        for (key, value) in table {
            if SCALAR_FIELDS.contains(&key.as_str()) {
                continue;
            }
            let version_table = value.as_table().ok_or_else(|| {
                Self::invalid(
                    file_id,
                    format!("manifest key `{key}` must be a version table"),
                )
            })?;
            for nested_key in version_table.keys() {
                if nested_key != "bindings" {
                    return Err(Self::invalid(
                        file_id,
                        format!(
                            "version table `{key}` contains unknown key `{nested_key}`; \
                             only `bindings` is allowed (a dotted version key must be \
                             quoted: `[\"1.0.0\".bindings]`, not `[1.0.0.bindings]`)"
                        ),
                    ));
                }
            }
            let Some(bindings_value) = version_table.get("bindings") else {
                continue;
            };
            let bindings_table = bindings_value.as_table().ok_or_else(|| {
                Self::invalid(
                    file_id,
                    format!("`bindings` of version `{key}` must be a table"),
                )
            })?;

            let mut version_bindings = Vec::new();
            for (pou, binding_value) in bindings_table {
                // `"declare-only"` is the only binding form. In particular a
                // manifest cannot select a native implementation: data files
                // must never direct code emission, so native behavior is
                // exposed as typed `__`-prefixed compiler intrinsics that
                // library ST bodies call instead.
                match binding_value {
                    toml::Value::String(text) if text == "declare-only" => {
                        version_bindings.push(pou.to_uppercase());
                    }
                    _ => {
                        return Err(Self::invalid(
                            file_id,
                            format!(
                                "binding for `{pou}` in version `{key}` must be \
                                 `\"declare-only\"` (the only supported binding)"
                            ),
                        ));
                    }
                }
            }
            all_bindings.insert(key.clone(), version_bindings);
        }
        Ok(all_bindings)
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

    // -----------------------------------------------------------------
    // Per-version bindings tables.
    // See specs/design/compatibility-library-format.md §Bindings and
    // specs/plans/2026-08-08-compatibility-library-bindings.md.
    // -----------------------------------------------------------------

    const IDENTITY: &str = "name = \"Tc2_Math\"\nvendor = \"ACME\"\ndefault_version = \"1.0.0\"\nreferences = [\"https://example.com\"]\n";

    #[test]
    fn from_toml_when_no_bindings_then_empty_map() {
        let manifest = LibraryManifest::from_toml(IDENTITY, &file_id()).unwrap();
        assert!(manifest.declare_only.is_empty());
    }

    #[test]
    fn from_toml_when_bindings_table_then_parses_declare_only() {
        let content =
            format!("{IDENTITY}\n[\"1.0.0\".bindings]\nMY_DECL_ONLY = \"declare-only\"\n");
        let manifest = LibraryManifest::from_toml(&content, &file_id()).unwrap();
        assert_eq!(
            manifest.declare_only.get("1.0.0"),
            Some(&vec!["MY_DECL_ONLY".to_string()])
        );
    }

    #[test]
    fn from_toml_when_binding_pou_lowercase_then_name_uppercased() {
        let content =
            format!("{IDENTITY}\n[\"1.0.0\".bindings]\nmy_decl_only = \"declare-only\"\n");
        let manifest = LibraryManifest::from_toml(&content, &file_id()).unwrap();
        assert!(manifest.declare_only["1.0.0"].contains(&"MY_DECL_ONLY".to_string()));
    }

    #[test]
    fn from_toml_when_unquoted_version_key_then_error() {
        // `[1.0.0.bindings]` is three nested TOML tables, not a version
        // table -- the nested key `0` is not `bindings`, so shape
        // validation rejects it.
        let content = format!("{IDENTITY}\n[1.0.0.bindings]\nMY_SQRT = \"declare-only\"\n");
        let err = LibraryManifest::from_toml(&content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_unknown_key_in_version_table_then_error() {
        let content = format!("{IDENTITY}\n[\"1.0.0\"]\nnot_bindings = 1\n");
        let err = LibraryManifest::from_toml(&content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_binding_value_unknown_string_then_error() {
        let content = format!("{IDENTITY}\n[\"1.0.0\".bindings]\nMY_SQRT = \"native\"\n");
        let err = LibraryManifest::from_toml(&content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_binding_is_intrinsic_table_then_error() {
        // A manifest cannot select a native implementation -- data files
        // must never direct code emission. The rejected earlier design's
        // `{ intrinsic = "..." }` form is malformed.
        let content = format!(
            "{IDENTITY}\n[\"1.0.0\".bindings]\nMY_SQRT = {{ intrinsic = \"sqrt_lreal\" }}\n"
        );
        let err = LibraryManifest::from_toml(&content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_binding_value_wrong_type_then_error() {
        let content = format!("{IDENTITY}\n[\"1.0.0\".bindings]\nMY_SQRT = 42\n");
        let err = LibraryManifest::from_toml(&content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_non_default_version_malformed_then_error() {
        // Shape validation covers every version table, not only the
        // default version.
        let content = format!("{IDENTITY}\n[\"2.0.0\".bindings]\nMY_SQRT = \"nope\"\n");
        let err = LibraryManifest::from_toml(&content, &file_id()).unwrap_err();
        assert_eq!(err.code, Problem::LibraryManifestInvalid.code());
    }

    #[test]
    fn from_toml_when_non_default_version_valid_then_inert_but_parsed() {
        let content = format!("{IDENTITY}\n[\"2.0.0\".bindings]\nMY_SQRT = \"declare-only\"\n");
        let manifest = LibraryManifest::from_toml(&content, &file_id()).unwrap();
        assert!(manifest.declare_only.contains_key("2.0.0"));
        assert!(!manifest.declare_only.contains_key("1.0.0"));
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
