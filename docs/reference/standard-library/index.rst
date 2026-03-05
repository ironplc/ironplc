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
     - Supported
   * - :doc:`SQRT <functions/sqrt>`
     - Square root
     - Supported (REAL, LREAL)
   * - :doc:`LN <functions/ln>`
     - Natural logarithm
     - Supported (REAL, LREAL)
   * - :doc:`LOG <functions/log>`
     - Base-10 logarithm
     - Supported (REAL, LREAL)
   * - :doc:`EXP <functions/exp>`
     - Natural exponential
     - Supported (REAL, LREAL)
   * - :doc:`EXPT <functions/expt>`
     - Exponentiation
     - Supported
   * - :doc:`SIN <functions/sin>`
     - Sine
     - Supported (REAL, LREAL)
   * - :doc:`COS <functions/cos>`
     - Cosine
     - Supported (REAL, LREAL)
   * - :doc:`TAN <functions/tan>`
     - Tangent
     - Supported (REAL, LREAL)
   * - :doc:`ASIN <functions/asin>`
     - Arc sine
     - Supported (REAL, LREAL)
   * - :doc:`ACOS <functions/acos>`
     - Arc cosine
     - Supported (REAL, LREAL)
   * - :doc:`ATAN <functions/atan>`
     - Arc tangent
     - Supported (REAL, LREAL)
   * - :doc:`ADD <functions/add>`
     - Addition
     - Supported
   * - :doc:`SUB <functions/sub>`
     - Subtraction
     - Supported
   * - :doc:`MUL <functions/mul>`
     - Multiplication
     - Supported
   * - :doc:`DIV <functions/div>`
     - Division
     - Supported
   * - :doc:`MOD <functions/mod>`
     - Modulo
     - Supported
   * - :doc:`GT <functions/gt>`
     - Greater than
     - Supported
   * - :doc:`GE <functions/ge>`
     - Greater than or equal
     - Supported
   * - :doc:`EQ <functions/eq>`
     - Equal
     - Supported
   * - :doc:`LE <functions/le>`
     - Less than or equal
     - Supported
   * - :doc:`LT <functions/lt>`
     - Less than
     - Supported
   * - :doc:`NE <functions/ne>`
     - Not equal
     - Supported
   * - :doc:`SEL <functions/sel>`
     - Binary selection
     - Supported
   * - :doc:`MAX <functions/max>`
     - Maximum
     - Supported
   * - :doc:`MIN <functions/min>`
     - Minimum
     - Supported
   * - :doc:`LIMIT <functions/limit>`
     - Clamp to range
     - Supported
   * - :doc:`MUX <functions/mux>`
     - Multiplexer
     - Supported
   * - :doc:`SHL <functions/shl>`
     - Shift left
     - Supported
   * - :doc:`SHR <functions/shr>`
     - Shift right
     - Supported
   * - :doc:`ROL <functions/rol>`
     - Rotate left
     - Supported
   * - :doc:`ROR <functions/ror>`
     - Rotate right
     - Supported
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
     - Supported

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
     - Supported
   * - :doc:`TOF <function-blocks/tof>`
     - Off-delay timer
     - Supported
   * - :doc:`TP <function-blocks/tp>`
     - Pulse timer
     - Supported
   * - :doc:`CTU <function-blocks/ctu>`
     - Count up
     - Supported
   * - :doc:`CTD <function-blocks/ctd>`
     - Count down
     - Supported
   * - :doc:`CTUD <function-blocks/ctud>`
     - Count up/down
     - Supported
   * - :doc:`R_TRIG <function-blocks/r-trig>`
     - Rising edge detection
     - Supported
   * - :doc:`F_TRIG <function-blocks/f-trig>`
     - Falling edge detection
     - Supported
   * - :doc:`SR <function-blocks/sr>`
     - Set/reset flip-flop
     - Supported
   * - :doc:`RS <function-blocks/rs>`
     - Reset/set flip-flop
     - Supported

.. toctree::
   :maxdepth: 1
   :hidden:

   functions/index
   function-blocks/index
