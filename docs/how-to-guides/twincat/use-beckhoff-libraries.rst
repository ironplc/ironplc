========================
Use Beckhoff Libraries
========================

This guide shows how IronPLC handles the Beckhoff libraries a TwinCAT 3
project references — and how to structure your own libraries so IronPLC
compiles them together with your application.

.. include:: ../../includes/requires-compiler.rst

-------------------------------------------
Referenced Libraries Work Automatically
-------------------------------------------

When your :file:`.plcproj` references a Beckhoff library — for example
``Tc2_System`` — IronPLC reads the reference from the project file and
activates its own bundled *compatibility library*: an independently
authored implementation of the same functions and constants, under their
exact names. You change nothing; code like this checks, compiles, and
runs as-is:

.. code-block::

   FUNCTION F_DegreesToRadians : LREAL
   VAR_INPUT
       degrees : LREAL;
   END_VAR
   F_DegreesToRadians := degrees * PI / 180.0;
   END_FUNCTION

Here ``PI`` comes from the ``Tc2_System`` reference in the project file.
A library is only activated when your project references it, matching
TwinCAT's own behavior.

The built-in conversion operators that TwinCAT provides to every project
without any reference — for example ``BOOL_TO_STRING`` — are always
available in TwinCAT projects through the implicit ``Tc2_BuiltIns``
compatibility library.

-------------------------------------------
Which Libraries Are Bundled
-------------------------------------------

Library support is early, and coverage grows release by release. IronPLC
currently bundles compatibility libraries for ``Tc2_System``,
``Tc2_Math``, ``Tc2_Utilities``, and the implicit ``Tc2_BuiltIns``. See
:doc:`/reference/compatibility-libraries/index` for exactly which
functions and constants each one provides.

If your project references a library that IronPLC does not bundle, the
check stops with problem
:doc:`P6011 </reference/compiler/problems/P6011>` naming the library, so
you know immediately which dependency is missing rather than getting a
cascade of undefined-symbol errors.

-------------------------------------------
Activate a Library for Loose Files
-------------------------------------------

When you check files without a :file:`.plcproj` — a single POU, or plain
Structured Text — there is no project file to read references from. Use
the ``--library`` option to activate a library explicitly:

.. code-block:: shell

   ironplcc check --dialect twincat --library Tc2_System MyFunction.TcPOU

The option is repeatable: pass ``--library`` once per library.

-------------------------------------------
Use Your Own Libraries
-------------------------------------------

If you factor shared code into your own PLC library project, keep the
library's *source* project inside the solution (or workspace) you give
IronPLC. IronPLC merges every :file:`.plcproj` it discovers into one
compilation unit, so your application resolves the library's functions,
function blocks, and types directly from their source:

.. code-block:: text

   MySolution/
   ├── MySolution.sln
   ├── App/
   │   ├── App.plcproj
   │   └── POUs/MAIN.TcPOU          (calls F_Scale)
   └── MyLib/
       ├── MyLib.plcproj
       └── POUs/F_Scale.TcPOU

IronPLC does not read precompiled or managed library files
(:file:`.library` / :file:`.compiled-library`), so a library that exists
only in TwinCAT's library repository is not visible to IronPLC — the
library's source must be present in the directory you check.

-------------------------------------------
About the Compatibility Libraries
-------------------------------------------

.. include:: ../../includes/compat-library-independence.rst

-------------------------------------------
See Also
-------------------------------------------

- :doc:`/reference/compatibility-libraries/index` — bundled libraries and
  the symbols each provides
- :doc:`check-twincat-projects` — checking the solution that references
  the libraries
