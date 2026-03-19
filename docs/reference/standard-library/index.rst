================
Standard Library
================

IronPLC provides the standard functions and function blocks defined by
IEC 61131-3. These are available in all programming languages.

.. tip::

   Examples on supported function pages are interactive — you can edit and run
   them directly in the `IronPLC Playground <https://playground.ironplc.com>`_.

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`ABS <functions/abs>`
     - Absolute value
   * - :doc:`SQRT <functions/sqrt>`
     - Square root
   * - :doc:`LN <functions/ln>`
     - Natural logarithm
   * - :doc:`LOG <functions/log>`
     - Base-10 logarithm
   * - :doc:`EXP <functions/exp>`
     - Natural exponential
   * - :doc:`EXPT <functions/expt>`
     - Exponentiation
   * - :doc:`SIN <functions/sin>`
     - Sine
   * - :doc:`COS <functions/cos>`
     - Cosine
   * - :doc:`TAN <functions/tan>`
     - Tangent
   * - :doc:`ASIN <functions/asin>`
     - Arc sine
   * - :doc:`ACOS <functions/acos>`
     - Arc cosine
   * - :doc:`ATAN <functions/atan>`
     - Arc tangent
   * - :doc:`ADD <functions/add>`
     - Addition
   * - :doc:`SUB <functions/sub>`
     - Subtraction
   * - :doc:`MUL <functions/mul>`
     - Multiplication
   * - :doc:`DIV <functions/div>`
     - Division
   * - :doc:`MOD <functions/mod>`
     - Modulo
   * - :doc:`GT <functions/gt>`
     - Greater than
   * - :doc:`GE <functions/ge>`
     - Greater than or equal
   * - :doc:`EQ <functions/eq>`
     - Equal
   * - :doc:`LE <functions/le>`
     - Less than or equal
   * - :doc:`LT <functions/lt>`
     - Less than
   * - :doc:`NE <functions/ne>`
     - Not equal
   * - :doc:`SEL <functions/sel>`
     - Binary selection
   * - :doc:`MAX <functions/max>`
     - Maximum
   * - :doc:`MIN <functions/min>`
     - Minimum
   * - :doc:`LIMIT <functions/limit>`
     - Clamp to range
   * - :doc:`MUX <functions/mux>`
     - Multiplexer
   * - :doc:`SHL <functions/shl>`
     - Shift left
   * - :doc:`SHR <functions/shr>`
     - Shift right
   * - :doc:`ROL <functions/rol>`
     - Rotate left
   * - :doc:`ROR <functions/ror>`
     - Rotate right
   * - :doc:`LEN <functions/len>`
     - String length
   * - :doc:`LEFT <functions/left>`
     - Left substring
   * - :doc:`RIGHT <functions/right>`
     - Right substring
   * - :doc:`MID <functions/mid>`
     - Middle substring
   * - :doc:`CONCAT <functions/concat>`
     - String concatenation
   * - :doc:`INSERT <functions/insert>`
     - String insertion
   * - :doc:`DELETE <functions/delete>`
     - String deletion
   * - :doc:`REPLACE <functions/replace>`
     - String replacement
   * - :doc:`FIND <functions/find>`
     - String search
   * - :doc:`Type conversions <functions/type-conversions>`
     - Type conversion functions

Function Blocks
---------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function Block
     - Description
   * - :doc:`TON <function-blocks/ton>`
     - On-delay timer
   * - :doc:`TOF <function-blocks/tof>`
     - Off-delay timer
   * - :doc:`TP <function-blocks/tp>`
     - Pulse timer
   * - :doc:`CTU <function-blocks/ctu>`
     - Count up
   * - :doc:`CTD <function-blocks/ctd>`
     - Count down
   * - :doc:`CTUD <function-blocks/ctud>`
     - Count up/down
   * - :doc:`R_TRIG <function-blocks/r-trig>`
     - Rising edge detection
   * - :doc:`F_TRIG <function-blocks/f-trig>`
     - Falling edge detection
   * - :doc:`SR <function-blocks/sr>`
     - Set/reset flip-flop
   * - :doc:`RS <function-blocks/rs>`
     - Reset/set flip-flop

.. toctree::
   :maxdepth: 1
   :hidden:

   functions/index
   function-blocks/index
