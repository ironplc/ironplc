=============
PLCopen XML
=============

IronPLC supports the PLCopen XML interchange format (TC6 XML) for importing
IEC 61131-3 programs from other development environments.

-----------------
Supported Version
-----------------

IronPLC supports **PLCopen TC6 XML version 2.01** (namespace: ``http://www.plcopen.org/xml/tc6_0201``).

-------------------
Supported Languages
-------------------

Within PLCopen XML files, IronPLC supports the following programming languages:

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

IronPLC supports the following PLCopen XML elements:

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
