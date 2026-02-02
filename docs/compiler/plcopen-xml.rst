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

If a PLCopen XML file contains POUs using unsupported languages, the compiler
will report error :doc:`P9003 <problems/P9003>`.

-----
Usage
-----

To check a PLCopen XML file, use the ``check`` command with the XML file path:

.. code-block:: shell

   ironplcc check myproject.xml

The compiler automatically detects PLCopen XML files by their content (XML with
the PLCopen namespace) regardless of file extension.

You can also check multiple files together, including a mix of PLCopen XML and
Structured Text files:

.. code-block:: shell

   ironplcc check types.st program.xml

This allows importing types or POUs from PLCopen XML while developing new code
in Structured Text.

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

-------------
Related Codes
-------------

The following problem codes are specific to PLCopen XML processing:

- :doc:`P0006 <problems/P0006>` - XML file is malformed
- :doc:`P0007 <problems/P0007>` - XML violates PLCopen schema
- :doc:`P0008 <problems/P0008>` - SFC body missing initial step
- :doc:`P6008 <problems/P6008>` - Unsupported PLCopen XML version
- :doc:`P9003 <problems/P9003>` - POU body language not supported
