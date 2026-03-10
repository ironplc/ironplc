==================
Variables and I/O
==================

This page explains how IEC 61131-3 programs declare variables and connect
them to physical inputs and outputs. For hands-on practice, see the
:doc:`/quickstart/index`.

--------------------------------------
Variable Declarations
--------------------------------------

Variables are declared inside :code:`VAR` ... :code:`END_VAR` blocks at the
top of a program, function, or function block:

.. code-block::

   VAR
      Counter : INT := 0;
      Temperature : REAL;
      MotorOn : BOOL := FALSE;
   END_VAR

Each declaration has a name, a type, and an optional initial value. If you
omit the initial value, the variable starts at the type's default (0 for
numbers, FALSE for BOOL, empty for STRING).

Variables declared inside a :code:`PROGRAM` or :code:`FUNCTION_BLOCK`
**retain their values between scans**. This is how you maintain state in a
cyclic program — you do not need global variables or external storage.

--------------------------------------
Variable Sections
--------------------------------------

IEC 61131-3 uses different keywords to indicate how a variable is used:

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Section
     - Meaning
   * - :code:`VAR`
     - Local variable, private to this POU.
   * - :code:`VAR_INPUT`
     - An input parameter passed into this POU by the caller.
   * - :code:`VAR_OUTPUT`
     - An output parameter passed back to the caller.
   * - :code:`VAR_IN_OUT`
     - A parameter passed by reference — the POU can read and modify it.
   * - :code:`VAR_GLOBAL`
     - A global variable visible across the configuration.

These sections make the data flow between program organization units
explicit, which is important for understanding and maintaining complex
control systems.

--------------------------------------
Directly Represented Variables
--------------------------------------

The key concept that distinguishes PLC programming from general-purpose
programming is the ability to map variables directly to hardware I/O
addresses. These are called **directly represented variables**.

A directly represented variable uses the :code:`AT` keyword followed by
an address:

.. code-block::

   VAR
      Button AT %IX1 : BOOL;
      Buzzer AT %QX1 : BOOL;
   END_VAR

The address follows a specific format:

.. code-block:: text

   %<direction><size><address>

Where:

- **Direction** indicates whether this is an input, output, or memory
  location:

  .. list-table::
     :header-rows: 1
     :widths: 15 85

     * - Prefix
       - Meaning
     * - ``I``
       - **Input** — read from a sensor or external device
     * - ``Q``
       - **Output** — write to an actuator or external device
     * - ``M``
       - **Memory** — internal storage, not connected to hardware

- **Size** indicates the width of the data:

  .. list-table::
     :header-rows: 1
     :widths: 15 85

     * - Suffix
       - Size
     * - ``X``
       - Single bit (BOOL)
     * - ``B``
       - Byte (8 bits)
     * - ``W``
       - Word (16 bits)
     * - ``D``
       - Double word (32 bits)

- **Address** is a numeric identifier for the specific I/O point.

For example:

- ``%IX1`` — read a single bit from input address 1
- ``%QX1`` — write a single bit to output address 1
- ``%IW3`` — read a 16-bit word from input address 3
- ``%MD10`` — a 32-bit memory location at address 10

--------------------------------------
How I/O Works in the Scan Cycle
--------------------------------------

During each scan cycle (see :doc:`what-is-iec-61131-3`):

1. The runtime reads all physical inputs and copies them into the input
   variables (``%I``).
2. Your program executes using those values.
3. The runtime copies output variables (``%Q``) to the physical outputs.

This means your program never reads a sensor or writes an actuator
directly. Instead, it works with a snapshot of the I/O state that is
refreshed on each scan. This approach (sometimes called **process image**)
ensures that all input values are consistent within a single scan — a
sensor value does not change halfway through your logic.

--------------------------------------
Next Steps
--------------------------------------

To see these concepts in action, work through the :doc:`/quickstart/index`
which builds up a program step by step. For details on all supported data
types and variable sections, see the :doc:`/reference/compiler/index`.
