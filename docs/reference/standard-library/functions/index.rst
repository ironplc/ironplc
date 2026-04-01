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
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`ABS <abs>`
     - Absolute value
   * - :doc:`SQRT <sqrt>`
     - Square root
   * - :doc:`LN <ln>`
     - Natural logarithm
   * - :doc:`LOG <log>`
     - Base-10 logarithm
   * - :doc:`EXP <exp>`
     - Natural exponential
   * - :doc:`EXPT <expt>`
     - Exponentiation
   * - :doc:`TRUNC <trunc>`
     - Truncate real to integer

Trigonometric Functions
-----------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`SIN <sin>`
     - Sine
   * - :doc:`COS <cos>`
     - Cosine
   * - :doc:`TAN <tan>`
     - Tangent
   * - :doc:`ASIN <asin>`
     - Arc sine
   * - :doc:`ACOS <acos>`
     - Arc cosine
   * - :doc:`ATAN <atan>`
     - Arc tangent

Arithmetic Functions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`ADD <add>`
     - Addition
   * - :doc:`SUB <sub>`
     - Subtraction
   * - :doc:`MUL <mul>`
     - Multiplication
   * - :doc:`DIV <div>`
     - Division
   * - :doc:`MOD <mod>`
     - Modulo

Comparison Functions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`GT <gt>`
     - Greater than
   * - :doc:`GE <ge>`
     - Greater than or equal
   * - :doc:`EQ <eq>`
     - Equal
   * - :doc:`LE <le>`
     - Less than or equal
   * - :doc:`LT <lt>`
     - Less than
   * - :doc:`NE <ne>`
     - Not equal

Assignment Functions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`MOVE <move>`
     - Assignment (copy value)

Selection Functions
-------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`SEL <sel>`
     - Binary selection
   * - :doc:`MAX <max>`
     - Maximum
   * - :doc:`MIN <min>`
     - Minimum
   * - :doc:`LIMIT <limit>`
     - Clamp to range
   * - :doc:`MUX <mux>`
     - Multiplexer

Bit String Functions
--------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`SHL <shl>`
     - Shift left
   * - :doc:`SHR <shr>`
     - Shift right
   * - :doc:`ROL <rol>`
     - Rotate left
   * - :doc:`ROR <ror>`
     - Rotate right

String Functions
----------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`LEN <len>`
     - String length
   * - :doc:`LEFT <left>`
     - Left substring
   * - :doc:`RIGHT <right>`
     - Right substring
   * - :doc:`MID <mid>`
     - Middle substring
   * - :doc:`CONCAT <concat>`
     - String concatenation
   * - :doc:`INSERT <insert>`
     - String insertion
   * - :doc:`DELETE <delete>`
     - String deletion
   * - :doc:`REPLACE <replace>`
     - String replacement
   * - :doc:`FIND <find>`
     - String search

Time and Date Functions
-----------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`ADD_TIME <add_time>`
     - Add two durations
   * - :doc:`SUB_TIME <sub_time>`
     - Subtract durations
   * - :doc:`MUL_TIME <mul_time>`
     - Scale duration by number
   * - :doc:`DIV_TIME <div_time>`
     - Divide duration by number
   * - :doc:`ADD_DT_TIME <add_dt_time>`
     - Add duration to date-and-time
   * - :doc:`ADD_TOD_TIME <add_tod_time>`
     - Add duration to time-of-day
   * - :doc:`SUB_DT_TIME <sub_dt_time>`
     - Subtract duration from date-and-time
   * - :doc:`SUB_TOD_TIME <sub_tod_time>`
     - Subtract duration from time-of-day
   * - :doc:`SUB_DT_DT <sub_dt_dt>`
     - Difference between two datetimes
   * - :doc:`SUB_DATE_DATE <sub_date_date>`
     - Difference between two dates
   * - :doc:`SUB_TOD_TOD <sub_tod_tod>`
     - Difference between two times-of-day
   * - :doc:`CONCAT_DATE_TOD <concat_date_tod>`
     - Combine date and time-of-day

Type Conversion Functions
-------------------------

.. list-table::
   :header-rows: 1
   :widths: 20 80

   * - Function
     - Description
   * - :doc:`Type conversions <type-conversions>`
     - Type conversion functions (``*_TO_*``)
   * - :doc:`BCD_TO_INT / INT_TO_BCD <bcd>`
     - BCD conversion functions

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
   add_time
   sub_time
   mul_time
   div_time
   add_dt_time
   add_tod_time
   sub_dt_time
   sub_tod_time
   sub_dt_dt
   sub_date_date
   sub_tod_tod
   concat_date_tod
   type-conversions
   trunc
   bcd
