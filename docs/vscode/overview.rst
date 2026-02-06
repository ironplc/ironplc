========
Overview
========

The IronPLC extension brings IEC 61131-3 Structured Text support to Visual Studio Code
and compatible editors. It provides real-time feedback as you write code, helping you
catch errors before deployment.

Current Capabilities
====================

The extension provides:

**Language Support**

* Syntax highlighting for Structured Text (``.st``, ``.iec`` files)
* Syntax highlighting for PLCopen XML files (detected by namespace)
* Automatic bracket matching for IEC 61131-3 keywords

**Real-time Analysis**

* Syntax error detection as you type
* Semantic analysis for type checking and validation
* Diagnostic messages with problem codes linking to documentation

**Editor Integration**

* "New Structured Text File" command in the File menu
* Language server protocol (LSP) integration for responsive editing

Architecture
============

The extension works by connecting your editor to the IronPLC compiler (``ironplcc``)
which runs as a language server. When you open or edit a Structured Text file:

1. The extension starts the compiler in language server mode
2. The compiler analyzes your code in the background
3. Diagnostics appear in the editor as you type
4. Syntax highlighting provides visual feedback on code structure

This architecture means the same analysis engine used for compilation also powers
your editing experience.
