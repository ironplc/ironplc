============================
Check TwinCAT 3 Projects
============================

This guide shows how to use IronPLC to check a Beckhoff TwinCAT 3 project
for correctness — without changing the project.

.. include:: ../../includes/requires-compiler.rst

-------------------------------------------
Check a Solution from the Command Line
-------------------------------------------

Point IronPLC at the directory that holds your :file:`.sln` file — or at
the :file:`.sln` itself — and select the ``twincat`` dialect:

.. code-block:: shell

   ironplcc check --dialect twincat path/to/my-solution
   ironplcc check --dialect twincat path/to/my-solution/MySolution.sln

IronPLC follows the solution layout the same way TwinCAT XAE does. It
reads the :file:`.sln` to find the :file:`.tsproj` files, each
:file:`.tsproj` to find the :file:`.plcproj` PLC projects, and each
:file:`.plcproj` to find the :file:`.TcPOU`, :file:`.TcGVL`,
:file:`.TcDUT`, and :file:`.TcIO` files to analyze. A solution that
contains more than one PLC project is checked as a whole, so code in one
project can use types and functions declared in another.

Name the directory that holds the :file:`.sln`, not a directory above it.
IronPLC does not search subdirectories for a project file. A directory
that holds no :file:`.sln` or :file:`.plcproj` is checked as a folder of
loose files instead, which ignores what the project files say — see
:doc:`/reference/compiler/source-formats/twincat` for the full rules.

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
