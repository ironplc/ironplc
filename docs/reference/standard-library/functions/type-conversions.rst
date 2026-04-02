================
Type Conversions
================

IEC 61131-3 defines a set of type conversion functions that convert
values between data types. Each function follows the naming pattern
``<source>_TO_<target>``.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.1
   * - **Support**
     - Supported (numeric and time/date conversions)

Conversion Categories
---------------------

Integer Widening
^^^^^^^^^^^^^^^^

Conversions from a smaller integer type to a larger integer type.
These conversions are always safe — no data is lost.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``SINT_TO_INT``
     - 8-bit to 16-bit signed
     - Supported
   * - ``SINT_TO_DINT``
     - 8-bit to 32-bit signed
     - Supported
   * - ``SINT_TO_LINT``
     - 8-bit to 64-bit signed
     - Supported
   * - ``INT_TO_DINT``
     - 16-bit to 32-bit signed
     - Supported
   * - ``INT_TO_LINT``
     - 16-bit to 64-bit signed
     - Supported
   * - ``DINT_TO_LINT``
     - 32-bit to 64-bit signed
     - Supported
   * - ``USINT_TO_UINT``
     - 8-bit to 16-bit unsigned
     - Supported
   * - ``USINT_TO_UDINT``
     - 8-bit to 32-bit unsigned
     - Supported
   * - ``USINT_TO_ULINT``
     - 8-bit to 64-bit unsigned
     - Supported
   * - ``UINT_TO_UDINT``
     - 16-bit to 32-bit unsigned
     - Supported
   * - ``UINT_TO_ULINT``
     - 16-bit to 64-bit unsigned
     - Supported
   * - ``UDINT_TO_ULINT``
     - 32-bit to 64-bit unsigned
     - Supported

Integer Narrowing
^^^^^^^^^^^^^^^^^

Conversions from a larger integer type to a smaller integer type.
These conversions may lose data if the value exceeds the range of
the target type.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``INT_TO_SINT``
     - 16-bit to 8-bit signed
     - Supported
   * - ``DINT_TO_SINT``
     - 32-bit to 8-bit signed
     - Supported
   * - ``DINT_TO_INT``
     - 32-bit to 16-bit signed
     - Supported
   * - ``LINT_TO_SINT``
     - 64-bit to 8-bit signed
     - Supported
   * - ``LINT_TO_INT``
     - 64-bit to 16-bit signed
     - Supported
   * - ``LINT_TO_DINT``
     - 64-bit to 32-bit signed
     - Supported
   * - ``UINT_TO_USINT``
     - 16-bit to 8-bit unsigned
     - Supported
   * - ``UDINT_TO_USINT``
     - 32-bit to 8-bit unsigned
     - Supported
   * - ``UDINT_TO_UINT``
     - 32-bit to 16-bit unsigned
     - Supported
   * - ``ULINT_TO_USINT``
     - 64-bit to 8-bit unsigned
     - Supported
   * - ``ULINT_TO_UINT``
     - 64-bit to 16-bit unsigned
     - Supported
   * - ``ULINT_TO_UDINT``
     - 64-bit to 32-bit unsigned
     - Supported

Signed/Unsigned Conversions
^^^^^^^^^^^^^^^^^^^^^^^^^^^

Conversions between signed and unsigned integer types of the same
or different sizes.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``SINT_TO_USINT``
     - Signed to unsigned 8-bit
     - Supported
   * - ``INT_TO_UINT``
     - Signed to unsigned 16-bit
     - Supported
   * - ``DINT_TO_UDINT``
     - Signed to unsigned 32-bit
     - Supported
   * - ``LINT_TO_ULINT``
     - Signed to unsigned 64-bit
     - Supported
   * - ``USINT_TO_SINT``
     - Unsigned to signed 8-bit
     - Supported
   * - ``UINT_TO_INT``
     - Unsigned to signed 16-bit
     - Supported
   * - ``UDINT_TO_DINT``
     - Unsigned to signed 32-bit
     - Supported
   * - ``ULINT_TO_LINT``
     - Unsigned to signed 64-bit
     - Supported

Integer to Real
^^^^^^^^^^^^^^^

Conversions from integer types to floating-point types. Large integer
values may lose precision when converted to ``REAL``.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``SINT_TO_REAL``
     - 8-bit signed to single-precision
     - Supported
   * - ``INT_TO_REAL``
     - 16-bit signed to single-precision
     - Supported
   * - ``DINT_TO_REAL``
     - 32-bit signed to single-precision
     - Supported
   * - ``LINT_TO_REAL``
     - 64-bit signed to single-precision
     - Supported
   * - ``SINT_TO_LREAL``
     - 8-bit signed to double-precision
     - Supported
   * - ``INT_TO_LREAL``
     - 16-bit signed to double-precision
     - Supported
   * - ``DINT_TO_LREAL``
     - 32-bit signed to double-precision
     - Supported
   * - ``LINT_TO_LREAL``
     - 64-bit signed to double-precision
     - Supported

Real to Integer
^^^^^^^^^^^^^^^

Conversions from floating-point types to integer types. The fractional
part is truncated.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``REAL_TO_SINT``
     - Single-precision to 8-bit signed
     - Supported
   * - ``REAL_TO_INT``
     - Single-precision to 16-bit signed
     - Supported
   * - ``REAL_TO_DINT``
     - Single-precision to 32-bit signed
     - Supported
   * - ``REAL_TO_LINT``
     - Single-precision to 64-bit signed
     - Supported
   * - ``LREAL_TO_SINT``
     - Double-precision to 8-bit signed
     - Supported
   * - ``LREAL_TO_INT``
     - Double-precision to 16-bit signed
     - Supported
   * - ``LREAL_TO_DINT``
     - Double-precision to 32-bit signed
     - Supported
   * - ``LREAL_TO_LINT``
     - Double-precision to 64-bit signed
     - Supported

Real to Real
^^^^^^^^^^^^^

Conversions between floating-point types.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``REAL_TO_LREAL``
     - Single-precision to double-precision
     - Supported
   * - ``LREAL_TO_REAL``
     - Double-precision to single-precision
     - Supported

Boolean Conversions
^^^^^^^^^^^^^^^^^^^

Conversions between ``BOOL`` and integer types. ``FALSE`` converts
to 0, ``TRUE`` converts to 1. For the reverse direction, 0 converts
to ``FALSE`` and any non-zero value converts to ``TRUE``.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``BOOL_TO_SINT``
     - Boolean to 8-bit signed
     - Supported
   * - ``BOOL_TO_INT``
     - Boolean to 16-bit signed
     - Supported
   * - ``BOOL_TO_DINT``
     - Boolean to 32-bit signed
     - Supported
   * - ``BOOL_TO_LINT``
     - Boolean to 64-bit signed
     - Supported
   * - ``BOOL_TO_USINT``
     - Boolean to 8-bit unsigned
     - Supported
   * - ``BOOL_TO_UINT``
     - Boolean to 16-bit unsigned
     - Supported
   * - ``BOOL_TO_UDINT``
     - Boolean to 32-bit unsigned
     - Supported
   * - ``BOOL_TO_ULINT``
     - Boolean to 64-bit unsigned
     - Supported
   * - ``SINT_TO_BOOL``
     - 8-bit signed to Boolean
     - Supported
   * - ``INT_TO_BOOL``
     - 16-bit signed to Boolean
     - Supported
   * - ``DINT_TO_BOOL``
     - 32-bit signed to Boolean
     - Supported
   * - ``LINT_TO_BOOL``
     - 64-bit signed to Boolean
     - Supported
   * - ``USINT_TO_BOOL``
     - 8-bit unsigned to Boolean
     - Supported
   * - ``UINT_TO_BOOL``
     - 16-bit unsigned to Boolean
     - Supported
   * - ``UDINT_TO_BOOL``
     - 32-bit unsigned to Boolean
     - Supported
   * - ``ULINT_TO_BOOL``
     - 64-bit unsigned to Boolean
     - Supported

Time/Duration Conversions
^^^^^^^^^^^^^^^^^^^^^^^^^

Conversions between time duration types (``TIME``, ``LTIME``) and
numeric or bit string types. The underlying value is the duration in
milliseconds.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``TIME_TO_DINT``
     - Duration to 32-bit signed
     - Supported
   * - ``TIME_TO_INT``
     - Duration to 16-bit signed
     - Supported
   * - ``TIME_TO_REAL``
     - Duration to single-precision float
     - Supported
   * - ``TIME_TO_DWORD``
     - Duration to 32-bit unsigned word
     - Supported
   * - ``DINT_TO_TIME``
     - 32-bit signed to duration
     - Supported
   * - ``DWORD_TO_TIME``
     - 32-bit unsigned word to duration
     - Supported
   * - ``LTIME_TO_LINT``
     - Long duration to 64-bit signed
     - Supported
   * - ``LTIME_TO_LWORD``
     - Long duration to 64-bit unsigned word
     - Supported
   * - ``LINT_TO_LTIME``
     - 64-bit signed to long duration
     - Supported

All combinations of ``TIME``/``LTIME`` with signed integers, unsigned
integers, real types, and bit string types are supported.

Date Conversions
^^^^^^^^^^^^^^^^

Conversions between date types (``DATE``, ``LDATE``) and numeric or
bit string types. The underlying value is seconds since 1970-01-01.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``DATE_TO_DWORD``
     - Date to 32-bit unsigned word
     - Supported
   * - ``DATE_TO_UDINT``
     - Date to 32-bit unsigned integer
     - Supported
   * - ``DWORD_TO_DATE``
     - 32-bit unsigned word to date
     - Supported
   * - ``LDATE_TO_LWORD``
     - Long date to 64-bit unsigned word
     - Supported

All combinations of ``DATE``/``LDATE`` with signed integers, unsigned
integers, real types, and bit string types are supported.

Time-of-Day Conversions
^^^^^^^^^^^^^^^^^^^^^^^

Conversions between time-of-day types (``TOD``/``TIME_OF_DAY``,
``LTOD``/``LTIME_OF_DAY``) and numeric or bit string types. The
underlying value is milliseconds since midnight (``TOD``) or
nanoseconds since midnight (``LTOD``).

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``TOD_TO_DWORD``
     - Time-of-day to 32-bit unsigned word
     - Supported
   * - ``TOD_TO_UDINT``
     - Time-of-day to 32-bit unsigned integer
     - Supported
   * - ``DWORD_TO_TOD``
     - 32-bit unsigned word to time-of-day
     - Supported
   * - ``LTOD_TO_LWORD``
     - Long time-of-day to 64-bit unsigned word
     - Supported

Both short aliases (``TOD``, ``LTOD``) and full names
(``TIME_OF_DAY``, ``LTIME_OF_DAY``) are supported. All combinations
with signed integers, unsigned integers, real types, and bit string
types are supported.

Date-and-Time Conversions
^^^^^^^^^^^^^^^^^^^^^^^^^

Conversions between date-and-time types (``DT``/``DATE_AND_TIME``,
``LDT``/``LDATE_AND_TIME``) and numeric or bit string types. The
underlying value is seconds since 1970-01-01 (``DT``) or nanoseconds
since 1970-01-01 (``LDT``).

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``DT_TO_DWORD``
     - Date-and-time to 32-bit unsigned word
     - Supported
   * - ``DT_TO_UDINT``
     - Date-and-time to 32-bit unsigned integer
     - Supported
   * - ``DWORD_TO_DT``
     - 32-bit unsigned word to date-and-time
     - Supported
   * - ``LDT_TO_LWORD``
     - Long date-and-time to 64-bit unsigned word
     - Supported

Both short aliases (``DT``, ``LDT``) and full names
(``DATE_AND_TIME``, ``LDATE_AND_TIME``) are supported. All
combinations with signed integers, unsigned integers, real types,
and bit string types are supported.

Numeric to String
^^^^^^^^^^^^^^^^^

Conversions from numeric types to string representation.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``SINT_TO_STRING``
     - 8-bit signed to string
     - Not yet supported
   * - ``INT_TO_STRING``
     - 16-bit signed to string
     - Not yet supported
   * - ``DINT_TO_STRING``
     - 32-bit signed to string
     - Not yet supported
   * - ``LINT_TO_STRING``
     - 64-bit signed to string
     - Not yet supported
   * - ``REAL_TO_STRING``
     - Single-precision to string
     - Not yet supported
   * - ``LREAL_TO_STRING``
     - Double-precision to string
     - Not yet supported

String to Numeric
^^^^^^^^^^^^^^^^^

Conversions from string representation to numeric types. The string
must contain a valid numeric literal for the target type.

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Function
     - Description
     - Support
   * - ``STRING_TO_SINT``
     - String to 8-bit signed
     - Not yet supported
   * - ``STRING_TO_INT``
     - String to 16-bit signed
     - Not yet supported
   * - ``STRING_TO_DINT``
     - String to 32-bit signed
     - Not yet supported
   * - ``STRING_TO_LINT``
     - String to 64-bit signed
     - Not yet supported
   * - ``STRING_TO_REAL``
     - String to single-precision
     - Not yet supported
   * - ``STRING_TO_LREAL``
     - String to double-precision
     - Not yet supported

Description
-----------

Type conversion functions explicitly convert values from one data type
to another. IEC 61131-3 does not perform implicit type conversions;
all conversions must use the appropriate ``*_TO_*`` function.

When a conversion may lose data (narrowing conversions), the behavior
depends on the implementation. Values that exceed the range of the
target type may be truncated or cause a runtime error.

Example
-------

.. playground-with-program::
   :vars: int_val : INT; real_val : REAL; big_val : DINT;

   int_val := REAL_TO_INT(REAL#3.14);     (* int_val = 3 *)
   real_val := INT_TO_REAL(42);            (* real_val = 42.0 *)
   big_val := INT_TO_DINT(1000);           (* big_val = 1000, widening *)

See Also
--------

- :doc:`/reference/language/data-types/index` — data type reference
