================
Standard Library
================

IronPLC provides the standard functions and function blocks defined by
IEC 61131-3. These are available in all programming languages.

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`ABS <functions/abs>`
     - Absolute value
     - Not yet supported
   * - :doc:`SQRT <functions/sqrt>`
     - Square root
     - Not yet supported
   * - :doc:`LN <functions/ln>`
     - Natural logarithm
     - Not yet supported
   * - :doc:`LOG <functions/log>`
     - Base-10 logarithm
     - Not yet supported
   * - :doc:`EXP <functions/exp>`
     - Natural exponential
     - Not yet supported
   * - :doc:`EXPT <functions/expt>`
     - Exponentiation
     - Supported (INT)
   * - :doc:`SIN <functions/sin>`
     - Sine
     - Not yet supported
   * - :doc:`COS <functions/cos>`
     - Cosine
     - Not yet supported
   * - :doc:`TAN <functions/tan>`
     - Tangent
     - Not yet supported
   * - :doc:`ASIN <functions/asin>`
     - Arc sine
     - Not yet supported
   * - :doc:`ACOS <functions/acos>`
     - Arc cosine
     - Not yet supported
   * - :doc:`ATAN <functions/atan>`
     - Arc tangent
     - Not yet supported
   * - :doc:`ADD <functions/add>`
     - Addition
     - Supported (INT)
   * - :doc:`SUB <functions/sub>`
     - Subtraction
     - Supported (INT)
   * - :doc:`MUL <functions/mul>`
     - Multiplication
     - Supported (INT)
   * - :doc:`DIV <functions/div>`
     - Division
     - Supported (INT)
   * - :doc:`MOD <functions/mod>`
     - Modulo
     - Supported (INT)
   * - :doc:`GT <functions/gt>`
     - Greater than
     - Supported (INT)
   * - :doc:`GE <functions/ge>`
     - Greater than or equal
     - Supported (INT)
   * - :doc:`EQ <functions/eq>`
     - Equal
     - Supported (INT)
   * - :doc:`LE <functions/le>`
     - Less than or equal
     - Supported (INT)
   * - :doc:`LT <functions/lt>`
     - Less than
     - Supported (INT)
   * - :doc:`NE <functions/ne>`
     - Not equal
     - Supported (INT)
   * - :doc:`SEL <functions/sel>`
     - Binary selection
     - Not yet supported
   * - :doc:`MAX <functions/max>`
     - Maximum
     - Not yet supported
   * - :doc:`MIN <functions/min>`
     - Minimum
     - Not yet supported
   * - :doc:`LIMIT <functions/limit>`
     - Clamp to range
     - Not yet supported
   * - :doc:`MUX <functions/mux>`
     - Multiplexer
     - Not yet supported
   * - :doc:`SHL <functions/shl>`
     - Shift left
     - Not yet supported
   * - :doc:`SHR <functions/shr>`
     - Shift right
     - Not yet supported
   * - :doc:`ROL <functions/rol>`
     - Rotate left
     - Not yet supported
   * - :doc:`ROR <functions/ror>`
     - Rotate right
     - Not yet supported
   * - :doc:`LEN <functions/len>`
     - String length
     - Not yet supported
   * - :doc:`LEFT <functions/left>`
     - Left substring
     - Not yet supported
   * - :doc:`RIGHT <functions/right>`
     - Right substring
     - Not yet supported
   * - :doc:`MID <functions/mid>`
     - Middle substring
     - Not yet supported
   * - :doc:`CONCAT <functions/concat>`
     - String concatenation
     - Not yet supported
   * - :doc:`INSERT <functions/insert>`
     - String insertion
     - Not yet supported
   * - :doc:`DELETE <functions/delete>`
     - String deletion
     - Not yet supported
   * - :doc:`REPLACE <functions/replace>`
     - String replacement
     - Not yet supported
   * - :doc:`FIND <functions/find>`
     - String search
     - Not yet supported
   * - :doc:`Type conversions <functions/type-conversions>`
     - Type conversion functions
     - Not yet supported

Function Blocks
---------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function Block
     - Description
     - Status
   * - :doc:`TON <function-blocks/ton>`
     - On-delay timer
     - Not yet supported
   * - :doc:`TOF <function-blocks/tof>`
     - Off-delay timer
     - Not yet supported
   * - :doc:`TP <function-blocks/tp>`
     - Pulse timer
     - Not yet supported
   * - :doc:`CTU <function-blocks/ctu>`
     - Count up
     - Not yet supported
   * - :doc:`CTD <function-blocks/ctd>`
     - Count down
     - Not yet supported
   * - :doc:`CTUD <function-blocks/ctud>`
     - Count up/down
     - Not yet supported
   * - :doc:`R_TRIG <function-blocks/r-trig>`
     - Rising edge detection
     - Not yet supported
   * - :doc:`F_TRIG <function-blocks/f-trig>`
     - Falling edge detection
     - Not yet supported
   * - :doc:`SR <function-blocks/sr>`
     - Set/reset flip-flop
     - Not yet supported
   * - :doc:`RS <function-blocks/rs>`
     - Reset/set flip-flop
     - Not yet supported

.. toctree::
   :maxdepth: 1
   :hidden:

   functions/index
   function-blocks/index
