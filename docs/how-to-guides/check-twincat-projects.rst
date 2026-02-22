============================
Check TwinCAT 3 Projects
============================

This guide shows how to use IronPLC to check a Beckhoff TwinCAT 3 project
for correctness.

.. note::
   This guide assumes you have installed the IronPLC Compiler. See
   :ref:`installation steps target` if you have not already installed it.

-----------------------------------
Check with the VS Code Extension
-----------------------------------

The IronPLC VS Code extension automatically recognizes TwinCAT file types
(:file:`.TcPOU`, :file:`.TcGVL`, :file:`.TcDUT`).

1. Open your TwinCAT project folder in VS Code.
2. Open any :file:`.TcPOU`, :file:`.TcGVL`, or :file:`.TcDUT` file.
3. The extension highlights errors and warnings in the editor as you type.

-------------------------------------------
Check with the Command Line
-------------------------------------------

You can check individual TwinCAT files:

.. code-block:: shell

   ironplcc check MyProgram.TcPOU

Or check an entire TwinCAT project by pointing to the directory containing
the :file:`.plcproj` file. IronPLC reads the project file to discover which
source files to analyze:

.. code-block:: shell

   ironplcc check path/to/my-twincat-project

-------------------------------------------
Supported TwinCAT File Types
-------------------------------------------

IronPLC supports these TwinCAT file types:

- :file:`.TcPOU` - Program Organization Units (programs, function blocks, functions)
- :file:`.TcGVL` - Global Variable Lists
- :file:`.TcDUT` - Data Unit Types (type declarations)

Only Structured Text implementations are supported. Function Block Diagram,
Ladder Diagram, and other graphical languages within TwinCAT files are not
analyzed.

See :doc:`/compiler/source-formats/twincat` for complete details on TwinCAT
format support.
