===============
Bytecode Viewer
===============

When you open an :file:`.iplc` bytecode file in Visual Studio Code, the IronPLC
extension displays a human-readable disassembly of the binary contents. The
viewer is read-only — it shows the compiled output but does not allow editing.

The viewer opens automatically for any file with the :file:`.iplc` extension.
No additional configuration is required.

.. note::

   The bytecode viewer requires the IronPLC compiler to be installed and
   available. It uses the compiler's language server to disassemble the file.
   If the compiler is not found, the viewer displays error :doc:`problems/E0002`.

Sections
========

The viewer organizes the disassembly into collapsible sections. Each section
can be expanded or collapsed by clicking its heading.

File Header
-----------

Displays metadata about the bytecode container:

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Field
     - Description
   * - Format Version
     - Version of the IPLC bytecode format
   * - Flags
     - Optional sections present in the file (Content Signature, Debug Section, Type Section)
   * - Functions
     - Number of functions defined
   * - Variables
     - Number of global variables
   * - Max Stack Depth
     - Maximum operand stack depth across all functions
   * - Max Call Depth
     - Maximum function call nesting depth
   * - FB Instances
     - Number of function block instances
   * - FB Types
     - Number of function block type definitions
   * - Arrays
     - Number of array declarations
   * - Input Image / Output Image / Memory Image
     - Size in bytes of the I/O and memory regions
   * - Content Hash
     - Hash of the bytecode content
   * - Source Hash
     - Hash of the original source files

Constant Pool
-------------

Lists the constants embedded in the bytecode. Each entry shows its index,
data type, and value.

Functions
---------

Each function section shows:

- **Max Stack Depth** — maximum operand stack depth for the function
- **Locals** — number of local variables
- **Bytecode** — size of the function's bytecode in bytes

Below the metadata is an instruction table with three columns:

- **Offset** — byte offset of the instruction (hexadecimal)
- **Opcode** — the operation name, color-coded by category
- **Operands** — arguments to the instruction, with comments where applicable

Opcode Colors
^^^^^^^^^^^^^

Opcodes are color-coded to make the disassembly easier to scan:

.. list-table::
   :header-rows: 1
   :widths: 20 30 50

   * - Color
     - Category
     - Examples
   * - Blue
     - Load operations
     - ``LOAD``, ``LOAD_CONST``
   * - Green
     - Store operations
     - ``STORE``, ``STORE_GLOBAL``
   * - Orange
     - Arithmetic
     - ``ADD``, ``SUB``, ``MUL``, ``DIV``
   * - Red
     - Control flow
     - ``RETURN``, ``CALL``, ``JMP``, ``BR``

Troubleshooting
===============

The viewer may display an error instead of the disassembly:

- :doc:`problems/E0002` — the compiler was not found or did not start in time.
  Verify the compiler is installed and, if needed, set the ``ironplc.path``
  setting (see :doc:`settings`).
- :doc:`problems/E0003` — the compiler could not disassemble the file. The
  file may be corrupted or produced by an incompatible compiler version. Try
  recompiling from source.

.. seealso::

   :doc:`build-tasks`
      How to compile source files into :file:`.iplc` bytecode from within VS Code.

   :doc:`/quickstart/compiling-and-running`
      Tutorial covering the compile-and-run workflow from the command line.
