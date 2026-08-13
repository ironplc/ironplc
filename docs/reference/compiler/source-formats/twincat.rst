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
- :file:`.TcIO` - Interface declarations (requires
  ``--allow-fb-inheritance``, which the ``twincat`` dialect enables
  automatically; see :doc:`/explanation/enabling-dialects-and-features`)

-------------------
Supported Languages
-------------------

.. include:: ../../../includes/supported-languages.rst

------------------
Supported Elements
------------------

.. include:: ../../../includes/supported-elements.rst

-----------------
Project Discovery
-----------------

When you point IronPLC at a directory, the compiler searches it
recursively for :file:`.plcproj` files — the marker of a TwinCAT 3 PLC
project. This means you can point IronPLC at any level of the Visual
Studio solution layout that TwinCAT XAE creates:

.. code-block:: text

   MySolution/
   ├── MySolution.sln
   └── MySolution/
       ├── MySolution.tsproj
       └── PlcProject/
           ├── PlcProject.plcproj
           └── POUs/
               ├── MAIN.TcPOU
               └── F_Helper.TcPOU

The :file:`.sln` and :file:`.tsproj` files are not themselves parsed;
discovery walks past them to each :file:`.plcproj`, which it reads to
determine the source files to analyze (the ``<Compile>`` items).

When discovery finds more than one :file:`.plcproj`, all discovered
projects are merged into a single compilation unit, so code in one
project can use declarations from another. A project entry that cannot
be resolved does not abort discovery of the remaining sources.

------------------
Library References
------------------

Discovery also reads each :file:`.plcproj` project's library references
(``<PlaceholderReference>`` and ``<LibraryReference>`` items). A
reference whose name matches a bundled
:doc:`compatibility library </reference/compatibility-libraries/index>`
activates that library automatically; a reference to a library IronPLC
does not bundle reports problem
:doc:`P6011 </reference/compiler/problems/P6011>`. Names are matched
exactly and case-sensitively; the declared version is not used to select
a package. Precompiled library files (:file:`.library` /
:file:`.compiled-library`) are not read.
