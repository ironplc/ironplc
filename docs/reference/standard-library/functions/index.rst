=========
Functions
=========

IEC 61131-3 defines a set of standard functions available in all
programming languages. Functions are stateless — they produce the same
output for the same inputs every time.

Numeric Functions
-----------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`ABS <abs>`
     - Absolute value
     - Supported
   * - :doc:`SQRT <sqrt>`
     - Square root
     - Supported
   * - :doc:`LN <ln>`
     - Natural logarithm
     - Supported
   * - :doc:`LOG <log>`
     - Base-10 logarithm
     - Supported
   * - :doc:`EXP <exp>`
     - Natural exponential
     - Supported
   * - :doc:`EXPT <expt>`
     - Exponentiation
     - Supported
   * - :doc:`TRUNC <trunc>`
     - Truncate real to integer
     - Supported

Trigonometric Functions
-----------------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`SIN <sin>`
     - Sine
     - Supported
   * - :doc:`COS <cos>`
     - Cosine
     - Supported
   * - :doc:`TAN <tan>`
     - Tangent
     - Supported
   * - :doc:`ASIN <asin>`
     - Arc sine
     - Supported
   * - :doc:`ACOS <acos>`
     - Arc cosine
     - Supported
   * - :doc:`ATAN <atan>`
     - Arc tangent
     - Supported

Arithmetic Functions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`ADD <add>`
     - Addition
     - Supported
   * - :doc:`SUB <sub>`
     - Subtraction
     - Supported
   * - :doc:`MUL <mul>`
     - Multiplication
     - Supported
   * - :doc:`DIV <div>`
     - Division
     - Supported
   * - :doc:`MOD <mod>`
     - Modulo
     - Supported

Comparison Functions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`GT <gt>`
     - Greater than
     - Supported
   * - :doc:`GE <ge>`
     - Greater than or equal
     - Supported
   * - :doc:`EQ <eq>`
     - Equal
     - Supported
   * - :doc:`LE <le>`
     - Less than or equal
     - Supported
   * - :doc:`LT <lt>`
     - Less than
     - Supported
   * - :doc:`NE <ne>`
     - Not equal
     - Supported

Assignment Functions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`MOVE <move>`
     - Assignment (copy value)
     - Supported

Selection Functions
-------------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`SEL <sel>`
     - Binary selection
     - Supported
   * - :doc:`MAX <max>`
     - Maximum
     - Supported
   * - :doc:`MIN <min>`
     - Minimum
     - Supported
   * - :doc:`LIMIT <limit>`
     - Clamp to range
     - Supported
   * - :doc:`MUX <mux>`
     - Multiplexer
     - Not yet supported

Bit String Functions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`SHL <shl>`
     - Shift left
     - Supported
   * - :doc:`SHR <shr>`
     - Shift right
     - Supported
   * - :doc:`ROL <rol>`
     - Rotate left
     - Supported
   * - :doc:`ROR <ror>`
     - Rotate right
     - Supported

String Functions
----------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`LEN <len>`
     - String length
     - Supported
   * - :doc:`LEFT <left>`
     - Left substring
     - Supported
   * - :doc:`RIGHT <right>`
     - Right substring
     - Supported
   * - :doc:`MID <mid>`
     - Middle substring
     - Supported
   * - :doc:`CONCAT <concat>`
     - String concatenation
     - Supported
   * - :doc:`INSERT <insert>`
     - String insertion
     - Supported
   * - :doc:`DELETE <delete>`
     - String deletion
     - Supported
   * - :doc:`REPLACE <replace>`
     - String replacement
     - Supported
   * - :doc:`FIND <find>`
     - String search
     - Supported

Type Conversion Functions
-------------------------

.. list-table::
   :header-rows: 1
   :widths: 20 50 30

   * - Function
     - Description
     - Status
   * - :doc:`Type conversions <type-conversions>`
     - Type conversion functions (``*_TO_*``)
     - Supported (numeric)
   * - :doc:`BCD_TO_INT / INT_TO_BCD <bcd>`
     - BCD conversion functions
     - Supported

.. toctree::
   :maxdepth: 1
   :hidden:

   abs
   sqrt
   ln
   log
   exp
   expt
   sin
   cos
   tan
   asin
   acos
   atan
   add
   sub
   mul
   div
   mod
   move
   gt
   ge
   eq
   le
   lt
   ne
   sel
   max
   min
   limit
   mux
   shl
   shr
   rol
   ror
   len
   left
   right
   mid
   concat
   insert
   delete
   replace
   find
   type-conversions
   trunc
   bcd
