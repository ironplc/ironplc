=======================
Compatibility Libraries
=======================

IronPLC bundles *compatibility libraries*: independently authored
implementations of functions, function blocks, and constants from vendor
PLC libraries, under their exact vendor names. They exist so that source
code written against a vendor's library checks, compiles, and runs in
IronPLC without changes.

Compatibility libraries stay dormant until **activated**. A library is
activated in one of three ways:

1. **Project reference** — a discovered project file (for example a
   TwinCAT :file:`.plcproj`) references the library by name. This is the
   normal path; see :doc:`/how-to-guides/twincat/use-beckhoff-libraries`.
2. **Explicit option** — ``ironplcc check --library <name>`` (repeatable),
   for sources without a project file.
3. **Implicit** — a library that the vendor's environment provides to
   every project with no reference (for example ``Tc2_BuiltIns``) is
   activated automatically whenever a project for that vendor is
   discovered.

Referencing a library that IronPLC does not bundle reports problem
:doc:`P6011 </reference/compiler/problems/P6011>`. Library names are
matched exactly and case-sensitively.

Bundled Libraries
-----------------

.. list-table::
   :header-rows: 1
   :widths: 25 60 15

   * - Library
     - Provides
     - Activation
   * - :doc:`Tc2_System <tc2-system>`
     - System constants (``PI``)
     - Reference
   * - :doc:`Tc2_Math <tc2-math>`
     - ``LREAL`` math functions (``LTRUNC``, ``LMOD``, ``MODABS``,
       ``FRAC``)
     - Reference
   * - :doc:`Tc2_Utilities <tc2-utilities>`
     - Formatting functions (``LREAL_TO_FMTSTR``)
     - Reference
   * - :doc:`Tc2_BuiltIns <tc2-builtins>`
     - TwinCAT's implicit built-in operators (``BOOL_TO_STRING``)
     - Implicit

.. toctree::
   :maxdepth: 1
   :hidden:

   Tc2_System <tc2-system>
   Tc2_Math <tc2-math>
   Tc2_Utilities <tc2-utilities>
   Tc2_BuiltIns <tc2-builtins>

Independence
------------

.. include:: ../../includes/compat-library-independence.rst
