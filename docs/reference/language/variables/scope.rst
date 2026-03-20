==============
Variable Scope
==============

IEC 61131-3 provides keywords to control the scope and direction of
variables within program organization units.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.4.3
   * - **Support**
     - Partial

Scope Keywords
--------------

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Keyword
     - Description
     - Status
   * - ``VAR``
     - Local variable
     - Supported
   * - ``VAR_INPUT``
     - Input parameter (read-only in callee)
     - Partial
   * - ``VAR_OUTPUT``
     - Output parameter (written by callee)
     - Partial
   * - ``VAR_IN_OUT``
     - In/out parameter (passed by reference)
     - Partial
   * - ``VAR_GLOBAL``
     - Global variable (accessible across POUs)
     - Supported
   * - ``VAR_EXTERNAL``
     - Reference to a global variable
     - Supported

Example
-------

.. code-block::

   FUNCTION_BLOCK MotorControl
       VAR_INPUT
           start : BOOL;
           stop : BOOL;
       END_VAR
       VAR_OUTPUT
           running : BOOL;
       END_VAR
       VAR
           internal_state : INT;
       END_VAR

       IF start AND NOT stop THEN
           running := TRUE;
       ELSIF stop THEN
           running := FALSE;
       END_IF;
   END_FUNCTION_BLOCK

Global Variables
----------------

Global variables are declared in a :code:`CONFIGURATION` block using
:code:`VAR_GLOBAL` and accessed from programs using :code:`VAR_EXTERNAL`.
The :code:`VAR_EXTERNAL` declaration must match the name and type of the
global variable it references.

.. playground::

   CONFIGURATION config
     VAR_GLOBAL
       MaxSpeed : INT := 100;
       Readings : ARRAY[1..3] OF INT := [10, 20, 30];
     END_VAR
     RESOURCE resource1 ON PLC
       TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
       PROGRAM plc_task_instance WITH plc_task : main;
     END_RESOURCE
   END_CONFIGURATION

   PROGRAM main
     VAR_EXTERNAL
       MaxSpeed : INT;
       Readings : ARRAY[1..3] OF INT;
     END_VAR
     VAR
       currentSpeed : INT;
       firstReading : INT;
     END_VAR
     currentSpeed := MaxSpeed;
     firstReading := Readings[1];
   END_PROGRAM

Top-Level Global Variables (Vendor Extension)
----------------------------------------------

.. include:: ../../../includes/requires-vendor-extension.rst

Many PLC vendors allow :code:`VAR_GLOBAL` blocks at the top level of a file,
outside of a :code:`CONFIGURATION` block. IronPLC supports this common
extension to improve compatibility with code written for other PLC
environments.

Enable with ``--allow-top-level-var-global`` or ``--allow-all`` on the
command line.

.. playground::

   VAR_GLOBAL
     MaxSpeed : INT := 100;
   END_VAR

   PROGRAM main
     VAR_EXTERNAL
       MaxSpeed : INT;
     END_VAR
     VAR
       currentSpeed : INT;
     END_VAR
     currentSpeed := MaxSpeed;
   END_PROGRAM

Programs access top-level globals the same way as configuration globals —
through :code:`VAR_EXTERNAL` declarations that match the name and type.

See Also
--------

- :doc:`declarations` — basic variable syntax
- :doc:`/reference/language/pous/function-block` — function blocks
- :doc:`/reference/language/pous/function` — functions
