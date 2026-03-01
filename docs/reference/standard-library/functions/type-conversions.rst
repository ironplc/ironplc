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
     - Not yet supported

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
     - Not yet supported
   * - ``SINT_TO_DINT``
     - 8-bit to 32-bit signed
     - Not yet supported
   * - ``SINT_TO_LINT``
     - 8-bit to 64-bit signed
     - Not yet supported
   * - ``INT_TO_DINT``
     - 16-bit to 32-bit signed
     - Not yet supported
   * - ``INT_TO_LINT``
     - 16-bit to 64-bit signed
     - Not yet supported
   * - ``DINT_TO_LINT``
     - 32-bit to 64-bit signed
     - Not yet supported
   * - ``USINT_TO_UINT``
     - 8-bit to 16-bit unsigned
     - Not yet supported
   * - ``USINT_TO_UDINT``
     - 8-bit to 32-bit unsigned
     - Not yet supported
   * - ``USINT_TO_ULINT``
     - 8-bit to 64-bit unsigned
     - Not yet supported
   * - ``UINT_TO_UDINT``
     - 16-bit to 32-bit unsigned
     - Not yet supported
   * - ``UINT_TO_ULINT``
     - 16-bit to 64-bit unsigned
     - Not yet supported
   * - ``UDINT_TO_ULINT``
     - 32-bit to 64-bit unsigned
     - Not yet supported

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
     - Not yet supported
   * - ``DINT_TO_SINT``
     - 32-bit to 8-bit signed
     - Not yet supported
   * - ``DINT_TO_INT``
     - 32-bit to 16-bit signed
     - Not yet supported
   * - ``LINT_TO_SINT``
     - 64-bit to 8-bit signed
     - Not yet supported
   * - ``LINT_TO_INT``
     - 64-bit to 16-bit signed
     - Not yet supported
   * - ``LINT_TO_DINT``
     - 64-bit to 32-bit signed
     - Not yet supported
   * - ``UINT_TO_USINT``
     - 16-bit to 8-bit unsigned
     - Not yet supported
   * - ``UDINT_TO_USINT``
     - 32-bit to 8-bit unsigned
     - Not yet supported
   * - ``UDINT_TO_UINT``
     - 32-bit to 16-bit unsigned
     - Not yet supported
   * - ``ULINT_TO_USINT``
     - 64-bit to 8-bit unsigned
     - Not yet supported
   * - ``ULINT_TO_UINT``
     - 64-bit to 16-bit unsigned
     - Not yet supported
   * - ``ULINT_TO_UDINT``
     - 64-bit to 32-bit unsigned
     - Not yet supported

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
     - Not yet supported
   * - ``INT_TO_UINT``
     - Signed to unsigned 16-bit
     - Not yet supported
   * - ``DINT_TO_UDINT``
     - Signed to unsigned 32-bit
     - Not yet supported
   * - ``LINT_TO_ULINT``
     - Signed to unsigned 64-bit
     - Not yet supported
   * - ``USINT_TO_SINT``
     - Unsigned to signed 8-bit
     - Not yet supported
   * - ``UINT_TO_INT``
     - Unsigned to signed 16-bit
     - Not yet supported
   * - ``UDINT_TO_DINT``
     - Unsigned to signed 32-bit
     - Not yet supported
   * - ``ULINT_TO_LINT``
     - Unsigned to signed 64-bit
     - Not yet supported

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
     - Not yet supported
   * - ``INT_TO_REAL``
     - 16-bit signed to single-precision
     - Not yet supported
   * - ``DINT_TO_REAL``
     - 32-bit signed to single-precision
     - Not yet supported
   * - ``LINT_TO_REAL``
     - 64-bit signed to single-precision
     - Not yet supported
   * - ``SINT_TO_LREAL``
     - 8-bit signed to double-precision
     - Not yet supported
   * - ``INT_TO_LREAL``
     - 16-bit signed to double-precision
     - Not yet supported
   * - ``DINT_TO_LREAL``
     - 32-bit signed to double-precision
     - Not yet supported
   * - ``LINT_TO_LREAL``
     - 64-bit signed to double-precision
     - Not yet supported

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
     - Not yet supported
   * - ``REAL_TO_INT``
     - Single-precision to 16-bit signed
     - Not yet supported
   * - ``REAL_TO_DINT``
     - Single-precision to 32-bit signed
     - Not yet supported
   * - ``REAL_TO_LINT``
     - Single-precision to 64-bit signed
     - Not yet supported
   * - ``LREAL_TO_SINT``
     - Double-precision to 8-bit signed
     - Not yet supported
   * - ``LREAL_TO_INT``
     - Double-precision to 16-bit signed
     - Not yet supported
   * - ``LREAL_TO_DINT``
     - Double-precision to 32-bit signed
     - Not yet supported
   * - ``LREAL_TO_LINT``
     - Double-precision to 64-bit signed
     - Not yet supported

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
     - Not yet supported
   * - ``LREAL_TO_REAL``
     - Double-precision to single-precision
     - Not yet supported

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
     - Not yet supported
   * - ``BOOL_TO_INT``
     - Boolean to 16-bit signed
     - Not yet supported
   * - ``BOOL_TO_DINT``
     - Boolean to 32-bit signed
     - Not yet supported
   * - ``BOOL_TO_LINT``
     - Boolean to 64-bit signed
     - Not yet supported
   * - ``SINT_TO_BOOL``
     - 8-bit signed to Boolean
     - Not yet supported
   * - ``INT_TO_BOOL``
     - 16-bit signed to Boolean
     - Not yet supported
   * - ``DINT_TO_BOOL``
     - 32-bit signed to Boolean
     - Not yet supported
   * - ``LINT_TO_BOOL``
     - 64-bit signed to Boolean
     - Not yet supported

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

.. code-block::

   int_val := REAL_TO_INT(REAL#3.14);     (* int_val = 3 *)
   real_val := INT_TO_REAL(42);            (* real_val = 42.0 *)
   str_val := INT_TO_STRING(100);          (* str_val = '100' *)
   big_val := INT_TO_DINT(1000);           (* big_val = 1000, widening *)

See Also
--------

- :doc:`/reference/language/data-types/index` — data type reference
