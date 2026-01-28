# XML Schemas

This directory contains XML schemas used for parsing PLCopen XML files.

## tc6_xml_v201.xsd

PLCopen TC6 XML Schema version 2.01 for IEC 61131-3 program interchange.

- **Version**: 2.01
- **Namespace**: `http://www.plcopen.org/xml/tc6_0201`
- **Source**: [PLCopen TC6 specification](https://plcopen.org/downloads/plcopen-xml-version-201-xsd)
- **Downloaded from**: https://github.com/fekaputra/gloze-x/blob/master/tc6_xml_v201.xsd
- **Date retrieved**: 2026-01-28

This schema is used as a reference for the hand-written Rust structs in
`compiler/sources/src/xml/schema.rs`. IronPLC implements a subset of the full
schema, focusing on Structured Text (ST) body support.

## References

- [PLCopen XML Technical Documentation](https://www.plcopen.org/system/files/downloads/tc6_xml_v201_technical_doc.pdf)
- [IEC 61131-10](https://webstore.iec.ch/publication/4556) - PLCopen XML exchange format (standardized version)
