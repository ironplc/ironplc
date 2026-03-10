//! File type detection and classification

use std::path::Path;

/// Represents the type of source file
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileType {
    /// Structured Text files (.st, .iec)
    StructuredText,
    /// XML files (.xml)
    Xml,
    /// TwinCAT files (.TcPOU, .TcGVL, .TcDUT)
    TwinCat,
    /// Unknown file type
    Unknown,
}

impl FileType {
    /// Determines the file type based on the file extension
    pub fn from_path(path: &Path) -> Self {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some(ext) if ext.eq_ignore_ascii_case("xml") => FileType::Xml,
            Some(ext) if ext.eq_ignore_ascii_case("st") => FileType::StructuredText,
            Some(ext) if ext.eq_ignore_ascii_case("iec") => FileType::StructuredText,
            Some(ext) if ext.eq_ignore_ascii_case("tcpou") => FileType::TwinCat,
            Some(ext) if ext.eq_ignore_ascii_case("tcgvl") => FileType::TwinCat,
            Some(ext) if ext.eq_ignore_ascii_case("tcdut") => FileType::TwinCat,
            _ => FileType::Unknown,
        }
    }

    /// Returns true if this file type is supported
    pub fn is_supported(&self) -> bool {
        matches!(
            self,
            FileType::StructuredText | FileType::Xml | FileType::TwinCat
        )
    }

    /// Determines the file type based on content inspection.
    ///
    /// This is useful when no file extension is available (e.g., playground input).
    /// Valid Structured Text never starts with `<`, so this is a reliable heuristic.
    pub fn from_content(content: &str) -> Self {
        let trimmed = content.trim_start();
        if trimmed.starts_with('<') {
            if trimmed.contains("TcPlcObject") {
                FileType::TwinCat
            } else {
                FileType::Xml
            }
        } else {
            FileType::StructuredText
        }
    }

    /// Returns the file extensions associated with this file type
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            FileType::StructuredText => &["st", "iec"],
            FileType::Xml => &["xml"],
            FileType::TwinCat => &["TcPOU", "TcGVL", "TcDUT"],
            FileType::Unknown => &[],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn file_type_from_path_st() {
        let path = PathBuf::from("test.st");
        assert_eq!(FileType::from_path(&path), FileType::StructuredText);
    }

    #[test]
    fn file_type_from_path_iec() {
        let path = PathBuf::from("test.iec");
        assert_eq!(FileType::from_path(&path), FileType::StructuredText);
    }

    #[test]
    fn file_type_from_path_xml() {
        let path = PathBuf::from("test.xml");
        assert_eq!(FileType::from_path(&path), FileType::Xml);
    }

    #[test]
    fn file_type_from_path_unknown() {
        let path = PathBuf::from("test.txt");
        assert_eq!(FileType::from_path(&path), FileType::Unknown);
    }

    #[test]
    fn file_type_from_path_tcpou() {
        let path = PathBuf::from("MAIN.TcPOU");
        assert_eq!(FileType::from_path(&path), FileType::TwinCat);
    }

    #[test]
    fn file_type_from_path_tcgvl() {
        let path = PathBuf::from("GVL_Main.TcGVL");
        assert_eq!(FileType::from_path(&path), FileType::TwinCat);
    }

    #[test]
    fn file_type_from_path_tcdut() {
        let path = PathBuf::from("ST_MyStruct.TcDUT");
        assert_eq!(FileType::from_path(&path), FileType::TwinCat);
    }

    #[test]
    fn file_type_case_insensitive() {
        let path = PathBuf::from("test.XML");
        assert_eq!(FileType::from_path(&path), FileType::Xml);

        let path = PathBuf::from("test.ST");
        assert_eq!(FileType::from_path(&path), FileType::StructuredText);

        let path = PathBuf::from("main.tcpou");
        assert_eq!(FileType::from_path(&path), FileType::TwinCat);
    }

    #[test]
    fn file_type_is_supported() {
        assert!(FileType::StructuredText.is_supported());
        assert!(FileType::Xml.is_supported());
        assert!(FileType::TwinCat.is_supported());
        assert!(!FileType::Unknown.is_supported());
    }

    #[test]
    fn from_content_when_plcopen_xml_then_returns_xml() {
        let content = r#"<?xml version="1.0" encoding="utf-8"?>
<project xmlns="http://www.plcopen.org/xml/tc6_0201">
</project>"#;
        assert_eq!(FileType::from_content(content), FileType::Xml);
    }

    #[test]
    fn from_content_when_twincat_xml_then_returns_twincat() {
        let content = r#"<?xml version="1.0" encoding="utf-8"?>
<TcPlcObject Version="1.1.0.1">
  <POU Name="MAIN" Id="{00000000-0000-0000-0000-000000000000}" SpecialFunc="None">
  </POU>
</TcPlcObject>"#;
        assert_eq!(FileType::from_content(content), FileType::TwinCat);
    }

    #[test]
    fn from_content_when_structured_text_then_returns_st() {
        let content = "PROGRAM Main\nEND_PROGRAM";
        assert_eq!(FileType::from_content(content), FileType::StructuredText);
    }

    #[test]
    fn from_content_when_leading_whitespace_xml_then_returns_xml() {
        let content = "  \n  <?xml version=\"1.0\"?>\n<project xmlns=\"http://www.plcopen.org/xml/tc6_0201\"/>";
        assert_eq!(FileType::from_content(content), FileType::Xml);
    }

    #[test]
    fn file_type_extensions() {
        assert_eq!(FileType::StructuredText.extensions(), &["st", "iec"]);
        assert_eq!(FileType::Xml.extensions(), &["xml"]);
        assert_eq!(FileType::TwinCat.extensions(), &["TcPOU", "TcGVL", "TcDUT"]);
        let empty: &[&str] = &[];
        assert_eq!(FileType::Unknown.extensions(), empty);
    }
}
