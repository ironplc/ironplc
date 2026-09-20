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

Point IronPLC at a TwinCAT project manifest — a :file:`.sln` or a
:file:`.plcproj` — or at the directory that holds exactly one manifest:

.. code-block:: shell

   ironplcc check --dialect twincat MySolution/MySolution.sln
   ironplcc check --dialect twincat MySolution

IronPLC reads the directory you name and nothing below it. It does not
search the tree for :file:`.plcproj` files. Instead it follows the chain
of references that TwinCAT XAE follows: a :file:`.sln` names its
:file:`.tsproj` files, each :file:`.tsproj` names its :file:`.plcproj`
files, and each :file:`.plcproj` names the source files to analyze (its
``<Compile>`` items).

.. code-block:: text

   MySolution/
   ├── MySolution.sln              (names MySolution/MySolution.tsproj)
   └── MySolution/
       ├── MySolution.tsproj       (names PlcProject/PlcProject.plcproj)
       └── PlcProject/
           ├── PlcProject.plcproj  (names POUs/MAIN.TcPOU, POUs/F_Helper.TcPOU)
           └── POUs/
               ├── MAIN.TcPOU
               └── F_Helper.TcPOU

For this layout, name :file:`MySolution/` or
:file:`MySolution/MySolution.sln` to check the whole solution, or
:file:`PlcProject/` or :file:`PlcProject/PlcProject.plcproj` to check that
one PLC project.

Every file in the chain is parsed, so every file in the chain must be
correct. A manifest that cannot be read, that is not well-formed, or that
names a file that does not exist reports
:doc:`P6012 </reference/compiler/problems/P6012>`. IronPLC reports the
broken manifest rather than looking for the sources another way: the
manifest is the record of which files belong to the project.

A :file:`.tsproj` is part of the chain but is not an entry point. Name the
:file:`.sln` above it or a :file:`.plcproj` below it.

Solutions with Several PLC Projects
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

When the chain names more than one :file:`.plcproj` — an application
project plus the library projects it uses — IronPLC merges them into a
single compilation unit, so code in one project can use declarations from
another. A ``<Compile>`` entry naming a file that does not exist is
reported as a problem, and IronPLC still analyzes the entries that do
resolve.

Directories That Name No Project
^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^

A directory holding *no* manifest — the solution's parent directory, or a
folder of loose TwinCAT files — is not a TwinCAT project. Neither is a
directory holding *more than one* manifest, because it names no single
project. In both cases IronPLC enumerates every supported file in the
directory and its subdirectories and analyzes them together.

That fallback differs from opening the project in two ways that matter:

- IronPLC does not read the ``<Compile>`` items, so it analyzes every
  supported file it finds — including files the project does not list,
  such as a POU excluded from the project or one left behind by a rename.
- IronPLC does not read the :file:`.plcproj` library references, so the
  referenced Beckhoff libraries are not activated. Use the ``--library``
  option to activate them explicitly (see
  :doc:`/how-to-guides/twincat/use-beckhoff-libraries`).

To analyze the project as TwinCAT defines it, name the manifest, or the
directory that holds it, rather than a directory further up the tree.

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
