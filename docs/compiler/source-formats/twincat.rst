=======
TwinCAT
=======

IronPLC supports Beckhoff TwinCAT 3 project files for checking IEC 61131-3
programs developed in the TwinCAT XAE environment.

---------------
File Extensions
---------------

IronPLC recognizes the following TwinCAT file extensions (case-insensitive):

- :file:`.TcPOU` - Program Organization Units (programs, function blocks, functions)
- :file:`.TcGVL` - Global Variable Lists
- :file:`.TcDUT` - Data Unit Types (type declarations)

-------------------
Supported Languages
-------------------

.. include:: ../../includes/supported-languages.rst

------------------
Supported Elements
------------------

.. include:: ../../includes/supported-elements.rst

-----------------
Project Discovery
-----------------

When you point IronPLC at a directory containing a TwinCAT project, the compiler
detects TwinCAT 3 projects by the presence of a :file:`.plcproj` file. It then
reads the project file to discover which source files to analyze.
