====
Text
====

IronPLC supports plain text files containing IEC 61131-3 Structured Text (ST)
programs. This is the native format for developing new IEC 61131-3 code.

--------------
File Extension
--------------

IronPLC recognizes files with the ``.st`` or ``.iec`` extension as Structured Text
source files.

-------------------
Supported Languages
-------------------

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

IronPLC supports the following elements in Structured Text files:

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
