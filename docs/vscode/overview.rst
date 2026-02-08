========
Overview
========

The IronPLC extension brings IEC 61131-3 Structured Text support to Visual Studio Code.
It provides real-time feedback as you write code, helping you catch errors before
deployment.

Supported Languages
===================

IEC 61131-3 Structured Text
---------------------------

The primary language supported by IronPLC.

:File Extensions: ``.st``, ``.iec``

The extension provides:

* Syntax highlighting for keywords, types, comments, strings, and numbers
* Automatic bracket matching for block keywords (``IF``/``END_IF``, ``FUNCTION``/``END_FUNCTION``, etc.)
* Real-time diagnostic analysis

PLCopen XML
-----------

XML format for IEC 61131-3 projects as defined by PLCopen TC6.

The extension detects PLCopen XML files by looking for the PLCopen namespace
in the opening XML tags. It provides XML syntax highlighting with embedded
Structured Text support inside CDATA sections.

Commands
========

New Structured Text File
------------------------

Creates a new untitled file with the Structured Text language mode pre-selected.

:Menu Location: File > New File > Structured Text File

To use this command:

1. Open the Command Palette (Ctrl+Shift+P or Cmd+Shift+P)
2. Type "New Structured Text File"
3. Press Enter

Diagnostics
===========

The extension reports diagnostics from the IronPLC compiler in real-time:

* **Errors** (red underline): Code that will not compile
* **Warnings** (yellow underline): Code that may indicate problems

Each diagnostic includes a problem code (e.g., P0001) linking to documentation
that explains the issue and how to resolve it. See :doc:`/compiler/problems/index`
for the complete list.

Architecture
============

The extension connects to the IronPLC compiler (``ironplcc``) which runs as a
language server. When you open or edit a Structured Text file:

1. The extension starts the compiler in language server mode
2. The compiler analyzes your code in the background
3. Diagnostics appear in the editor as you type
4. Syntax highlighting provides visual feedback on code structure

This architecture means the same analysis engine used for compilation also powers
your editing experience.
