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

1. Open your TwinCAT project folder in VS Code.
2. Open any TwinCAT source file.
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

See :doc:`/compiler/source-formats/twincat` for supported file types and
format details.
