==================
Features Reference
==================

This page documents the features available in the IronPLC extension.

Supported Languages
===================

IEC 61131-3 Structured Text
---------------------------

The primary language supported by IronPLC.

:Language ID: ``61131-3-st``
:File Extensions: ``.st``, ``.iec``
:Aliases: "IEC 61131-3", "Structured Text"

The extension provides:

* TextMate-based syntax highlighting for keywords, types, comments, strings, and numbers
* Automatic bracket matching for block keywords (``IF``/``END_IF``, ``FUNCTION``/``END_FUNCTION``, etc.)
* Real-time diagnostic analysis

PLCopen XML
-----------

XML format for IEC 61131-3 projects as defined by PLCopen TC6.

:Language ID: ``plcopen-xml``
:File Extensions: Detected by content (XML namespace)
:Aliases: "PLCopen XML", "IEC 61131-3 XML", "TC6 XML"

The extension detects PLCopen XML files by looking for the PLCopen namespace
(``http://www.plcopen.org/xml/tc6``) in the opening XML tags.

Features:

* XML syntax highlighting with embedded Structured Text support
* CDATA section handling for ST code blocks

Commands
========

New Structured Text File
------------------------

Creates a new untitled file with the Structured Text language mode pre-selected.

:Command ID: ``ironplc.createNewStructuredTextFile``
:Menu Location: File > New File > Structured Text File
:Availability: Not available in virtual workspaces

To use this command:

1. Open the Command Palette (Ctrl+Shift+P or Cmd+Shift+P)
2. Type "New Structured Text File"
3. Press Enter

Alternatively, use File > New File and select "Structured Text File" from the list.

Diagnostics
===========

The extension reports diagnostics from the IronPLC compiler in real-time. Diagnostics
appear as:

* **Errors** (red underline): Code that will not compile
* **Warnings** (yellow underline): Code that may indicate problems

Each diagnostic includes:

* A problem code (e.g., P0001) linking to detailed documentation
* A description of the issue
* The location in your code

Click on a problem code in the Problems panel to open the corresponding documentation
page explaining the issue and how to resolve it.

For the complete list of compiler problem codes, see :doc:`/compiler/problems/index`.

Semantic Tokens
===============

The extension provides semantic token information to the editor, enabling richer
syntax highlighting based on the semantic meaning of code elements rather than
just textual patterns.

This means the editor can distinguish between:

* Variable names vs. keywords
* Type names vs. function names
* Different categories of identifiers based on their declaration
