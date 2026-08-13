============================
Check TwinCAT 3 Projects
============================

This guide shows how to use IronPLC to check a Beckhoff TwinCAT 3 project
for correctness — without changing the project.

.. include:: ../../includes/requires-compiler.rst

-------------------------------------------
Check a Solution from the Command Line
-------------------------------------------

Point IronPLC at your solution directory and select the ``twincat``
dialect:

.. code-block:: shell

   ironplcc check --dialect twincat path/to/my-solution

IronPLC walks the solution layout the same way TwinCAT XAE does: past the
:file:`.sln` and :file:`.tsproj` files, into each :file:`.plcproj` PLC
project, which it reads to discover the :file:`.TcPOU`, :file:`.TcGVL`,
:file:`.TcDUT`, and :file:`.TcIO` files to analyze. A solution that
contains more than one PLC project is checked as a whole, so code in one
project can use types and functions declared in another.

On success, the command produces no output. If there are problems, IronPLC
prints diagnostics with the file name, line number, and a description of
each problem.

The ``--dialect twincat`` option matters: it tells IronPLC to accept the
language extensions that TwinCAT accepts, so TwinCAT-specific syntax works
by default and you do not need to enable individual features. See
:doc:`/explanation/enabling-dialects-and-features` for how dialects work.

If your project references Beckhoff libraries such as ``Tc2_System``,
IronPLC activates its bundled compatibility libraries automatically — see
:doc:`use-beckhoff-libraries`.

-------------------------------------------
Check a Single File
-------------------------------------------

You can also check individual TwinCAT files:

.. code-block:: shell

   ironplcc check --dialect twincat MyProgram.TcPOU

Checking a single file is useful for a quick look, but prefer checking the
solution directory: only there can IronPLC resolve names declared in other
files and read the project's library references.

-----------------------------------
Check with the VS Code Extension
-----------------------------------

1. Open your TwinCAT solution folder in VS Code.
2. In the settings (:menuselection:`File --> Preferences --> Settings`),
   set :guilabel:`Ironplc: Dialect` to ``twincat``.
3. Open any TwinCAT source file. The extension highlights errors and
   warnings in the editor as you type.

-------------------------------------------
See Also
-------------------------------------------

- :doc:`/reference/compiler/source-formats/twincat` — supported file types
  and how project discovery works
- :doc:`/explanation/enabling-dialects-and-features` — what the ``twincat``
  dialect enables
- :doc:`run-twincat-projects` — the next step: execute the project
