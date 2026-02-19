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
    fn file_type_extensions() {
        assert_eq!(FileType::StructuredText.extensions(), &["st", "iec"]);
        assert_eq!(FileType::Xml.extensions(), &["xml"]);
        assert_eq!(FileType::TwinCat.extensions(), &["TcPOU", "TcGVL", "TcDUT"]);
        let empty: &[&str] = &[];
        assert_eq!(FileType::Unknown.extensions(), empty);
    }
}
