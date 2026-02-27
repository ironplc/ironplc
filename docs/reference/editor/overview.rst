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

:File Extensions: :file:`.st`, :file:`.iec`

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

TwinCAT
-------

Beckhoff TwinCAT 3 project files containing IEC 61131-3 source code.

:File Extensions: :file:`.TcPOU`, :file:`.TcGVL`, :file:`.TcDUT`

The extension provides XML syntax highlighting with embedded Structured Text
support and real-time diagnostic analysis.

Commands
========

New Structured Text File
------------------------

Creates a new untitled file with the Structured Text language mode pre-selected.

:Menu Location: :menuselection:`File --> New File... --> Structured Text File`

To use this command:

1. Open the Command Palette (:kbd:`Ctrl+Shift+P` or :kbd:`Cmd+Shift+P`)
2. Type "New Structured Text File"
3. Press Enter

Build Tasks
===========

The extension provides a build task that compiles your project to a bytecode
container (``.iplc``) file. Use :kbd:`Ctrl+Shift+B` to run the build task.
See :doc:`build-tasks` for details.

Bytecode Viewer
===============

Opening an :file:`.iplc` bytecode file displays a human-readable disassembly of
the compiled program, including the file header, constant pool, and function
instructions with color-coded opcodes. See :doc:`bytecode-viewer` for details.

Diagnostics
===========

The extension reports diagnostics from the IronPLC compiler in real-time:

* **Errors** (red underline): Code that will not compile
* **Warnings** (yellow underline): Code that may indicate problems

Each diagnostic includes a problem code (e.g., P0001) linking to documentation
that explains the issue and how to resolve it. See :doc:`/reference/compiler/problems/index`
for the complete list.
