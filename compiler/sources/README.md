# IronPLC Sources Module

The `ironplc-sources` module provides a unified interface for handling different types of source files in the IronPLC compiler. This module was created to better separate concerns and provide extensible support for multiple file formats.

## Architecture Overview

The module is organized around several key concepts:

### Core Components

1. **FileType** (`file_type.rs`): Enum representing different supported file types
   - `StructuredText`: For `.st` and `.iec` files
   - `Xml`: For `.xml` files  
   - `Unknown`: For unsupported file types

2. **Source** (`source.rs`): Abstraction for a single source file with parsing capabilities
   - Handles file content and metadata
   - Provides lazy parsing with caching
   - Automatically detects file type from extension

3. **SourceProject** (`project.rs`): Collection of source files that can be analyzed together
   - Manages multiple source files
   - Supports directory initialization
   - Provides unified access to all sources

4. **Parsers** (`parsers/`): Static parser functions for different file formats
   - `st_parser::parse()`: Handles ST/IEC files using existing parser
   - `xml_parser::parse()`: Currently returns empty Library (as requested)
   - `parse_source()`: Dispatches to appropriate parser based on file type
