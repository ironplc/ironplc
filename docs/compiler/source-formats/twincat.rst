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

Within TwinCAT files, IronPLC supports the following programming languages:

**Fully Supported:**

- **Structured Text (ST)** - Text-based programming language
- **Sequential Function Chart (SFC)** - State-machine based programming with ST action bodies

**Not Supported:**

- **Function Block Diagram (FBD)** - Graphical language
- **Ladder Diagram (LD)** - Graphical language
- **Instruction List (IL)** - Deprecated text-based language

------------------
Supported Elements
------------------

IronPLC supports the following elements in TwinCAT files:

**Data Types:**

- Elementary types (BOOL, INT, REAL, STRING, etc.)
- Enumeration types
- Array types (single and multi-dimensional)
- Structure types
- Subrange types
- Type aliases (derived types)

**Program Organization Units:**

- Functions
- Function Blocks
- Programs

**Configuration:**

- Configurations
- Resources
- Tasks
- Program instances

**SFC Elements:**

- Steps (including initial step)
- Transitions with ST conditions
- Actions with ST bodies
- Action associations with qualifiers (N, R, S, L, D, P)

-----------------
Project Discovery
-----------------

When you point IronPLC at a directory containing a TwinCAT project, the compiler
detects TwinCAT 3 projects by the presence of a :file:`.plcproj` file. It then
reads the project file to discover which source files to analyze.
