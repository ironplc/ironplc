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
     - Not yet supported
   * - :doc:`SQRT <sqrt>`
     - Square root
     - Not yet supported
   * - :doc:`LN <ln>`
     - Natural logarithm
     - Not yet supported
   * - :doc:`LOG <log>`
     - Base-10 logarithm
     - Not yet supported
   * - :doc:`EXP <exp>`
     - Natural exponential
     - Not yet supported
   * - :doc:`EXPT <expt>`
     - Exponentiation
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
     - Not yet supported
   * - :doc:`COS <cos>`
     - Cosine
     - Not yet supported
   * - :doc:`TAN <tan>`
     - Tangent
     - Not yet supported
   * - :doc:`ASIN <asin>`
     - Arc sine
     - Not yet supported
   * - :doc:`ACOS <acos>`
     - Arc cosine
     - Not yet supported
   * - :doc:`ATAN <atan>`
     - Arc tangent
     - Not yet supported

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
     - Not yet supported
   * - :doc:`MAX <max>`
     - Maximum
     - Not yet supported
   * - :doc:`MIN <min>`
     - Minimum
     - Not yet supported
   * - :doc:`LIMIT <limit>`
     - Clamp to range
     - Not yet supported
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
     - Not yet supported
   * - :doc:`SHR <shr>`
     - Shift right
     - Not yet supported
   * - :doc:`ROL <rol>`
     - Rotate left
     - Not yet supported
   * - :doc:`ROR <ror>`
     - Rotate right
     - Not yet supported

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
     - Not yet supported
   * - :doc:`LEFT <left>`
     - Left substring
     - Not yet supported
   * - :doc:`RIGHT <right>`
     - Right substring
     - Not yet supported
   * - :doc:`MID <mid>`
     - Middle substring
     - Not yet supported
   * - :doc:`CONCAT <concat>`
     - String concatenation
     - Not yet supported
   * - :doc:`INSERT <insert>`
     - String insertion
     - Not yet supported
   * - :doc:`DELETE <delete>`
     - String deletion
     - Not yet supported
   * - :doc:`REPLACE <replace>`
     - String replacement
     - Not yet supported
   * - :doc:`FIND <find>`
     - String search
     - Not yet supported

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
     - Not yet supported

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
