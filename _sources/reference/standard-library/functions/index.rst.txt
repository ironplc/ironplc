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
   * - :doc:`AND <and>`
     - Bitwise AND
   * - :doc:`OR <or>`
     - Bitwise OR
   * - :doc:`XOR <xor>`
     - Bitwise exclusive OR
   * - :doc:`NOT <not>`
     - Bitwise complement
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
   * - :doc:`ADD_LTIME <add_ltime>`
     - Add two long durations
   * - :doc:`SUB_LTIME <sub_ltime>`
     - Subtract long durations
   * - :doc:`MUL_LTIME <mul_ltime>`
     - Scale long duration by number
   * - :doc:`DIV_LTIME <div_ltime>`
     - Divide long duration by number
   * - :doc:`ADD_LDT_LTIME <add_ldt_ltime>`
     - Add long duration to long date-and-time
   * - :doc:`ADD_LTOD_LTIME <add_ltod_ltime>`
     - Add long duration to long time-of-day
   * - :doc:`SUB_LDT_LTIME <sub_ldt_ltime>`
     - Subtract long duration from long date-and-time
   * - :doc:`SUB_LTOD_LTIME <sub_ltod_ltime>`
     - Subtract long duration from long time-of-day
   * - :doc:`SUB_LDT_LDT <sub_ldt_ldt>`
     - Difference between two long datetimes
   * - :doc:`SUB_LDATE_LDATE <sub_ldate_ldate>`
     - Difference between two long dates
   * - :doc:`SUB_LTOD_LTOD <sub_ltod_ltod>`
     - Difference between two long times-of-day
   * - :doc:`CONCAT_DATE_TOD <concat_date_tod>`
     - Combine date and time-of-day
   * - :doc:`DT_TO_DATE <dt_to_date>`
     - Extract date from datetime
   * - :doc:`DT_TO_TOD <dt_to_tod>`
     - Extract time-of-day from datetime

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
   and
   or
   xor
   not
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
   add_ltime
   sub_ltime
   mul_ltime
   div_ltime
   add_ldt_ltime
   add_ltod_ltime
   sub_ldt_ltime
   sub_ltod_ltime
   sub_ldt_ldt
   sub_ldate_ldate
   sub_ltod_ltod
   concat_date_tod
   dt_to_date
   dt_to_tod
   type-conversions
   trunc
   bcd
