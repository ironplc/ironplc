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
     - Not yet supported
   * - ``VAR_EXTERNAL``
     - Reference to a global variable
     - Not yet supported

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

See Also
--------

- :doc:`declarations` — basic variable syntax
- :doc:`/reference/language/pous/function-block` — function blocks
- :doc:`/reference/language/pous/function` — functions
