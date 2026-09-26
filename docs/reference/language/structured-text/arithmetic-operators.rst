====================
Arithmetic Operators
====================

Arithmetic operators perform mathematical computations on numeric values.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.1
   * - **Support**
     - Supported

Syntax
------

.. list-table::
   :header-rows: 1
   :widths: 15 30 55

   * - Operator
     - Syntax
     - Description
   * - ``+``
     - ``a + b``
     - Addition
   * - ``-``
     - ``a - b``
     - Subtraction
   * - ``*``
     - ``a * b``
     - Multiplication
   * - ``/``
     - ``a / b``
     - Division (integer division for integer types)
   * - ``MOD``
     - ``a MOD b``
     - Modulo (remainder after division)
   * - ``**``
     - ``a ** b``
     - Exponentiation (power)
   * - ``-``
     - ``-a``
     - Unary negation

Description
-----------

Arithmetic operators apply to integer types (``SINT``, ``INT``, ``DINT``,
``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``) and floating-point types
(``REAL``, ``LREAL``). For integer division, the result is truncated toward
zero. The ``MOD`` operator returns the remainder of integer division and is
defined only for integer types.

When the two operands have different numeric types, the narrower one widens
to the wider, and the operation is carried out at the wider type, which is
also the result's type: ``INT + DINT`` is a ``DINT``, and ``INT + REAL`` is a
``REAL``. Two types where neither widens to the other, such as ``DINT`` and
``REAL``, are an error (:doc:`P4049 </reference/compiler/problems/P4049>`).
See :doc:`/explanation/type-conversions`.

``+``, ``-``, ``*`` and ``/`` are also defined on the time and date types,
each combination being one of the typed time functions:

.. list-table::
   :header-rows: 1
   :widths: 40 30 30

   * - Expression
     - Result
     - Same as
   * - ``TIME + TIME``, ``TIME - TIME``
     - ``TIME``
     - :doc:`ADD_TIME </reference/standard-library/functions/add_time>`,
       :doc:`SUB_TIME </reference/standard-library/functions/sub_time>`
   * - ``TIME_OF_DAY + TIME``, ``TIME_OF_DAY - TIME``
     - ``TIME_OF_DAY``
     - :doc:`ADD_TOD_TIME </reference/standard-library/functions/add_tod_time>`,
       :doc:`SUB_TOD_TIME </reference/standard-library/functions/sub_tod_time>`
   * - ``DATE_AND_TIME + TIME``, ``DATE_AND_TIME - TIME``
     - ``DATE_AND_TIME``
     - :doc:`ADD_DT_TIME </reference/standard-library/functions/add_dt_time>`,
       :doc:`SUB_DT_TIME </reference/standard-library/functions/sub_dt_time>`
   * - ``DATE - DATE``
     - ``TIME``
     - :doc:`SUB_DATE_DATE </reference/standard-library/functions/sub_date_date>`
   * - ``TIME_OF_DAY - TIME_OF_DAY``
     - ``TIME``
     - :doc:`SUB_TOD_TOD </reference/standard-library/functions/sub_tod_tod>`
   * - ``DATE_AND_TIME - DATE_AND_TIME``
     - ``TIME``
     - :doc:`SUB_DT_DT </reference/standard-library/functions/sub_dt_dt>`
   * - ``TIME * number``, ``TIME / number``
     - ``TIME``
     - :doc:`MUL_TIME </reference/standard-library/functions/mul_time>`,
       :doc:`DIV_TIME </reference/standard-library/functions/div_time>`

The long types (``LTIME``, ``LDATE``, ``LTIME_OF_DAY``, ``LDATE_AND_TIME``)
combine the same way, with each type in the table replaced by its long form;
a combination with at least one long operand has the long result.
Arithmetic on bit-string types requires ``--allow-bit-string-arithmetic``;
see :doc:`/explanation/type-conversions`.

The unary negation operator ``-`` has higher precedence than the binary
arithmetic operators. Exponentiation ``**`` has higher precedence than
multiplication, division, and modulo.

Example
-------

.. playground::

   PROGRAM main
       VAR
           a : INT := 17;
           b : INT := 5;
           sum : INT;
           diff : INT;
           product : INT;
           quotient : INT;
           remainder : INT;
       END_VAR

       sum := a + b;          (* 22 *)
       diff := a - b;         (* 12 *)
       product := a * b;      (* 85 *)
       quotient := a / b;     (* 3 *)
       remainder := a MOD b;  (* 2 *)
   END_PROGRAM

See Also
--------

- :doc:`comparison-operators` — relational operators
- :doc:`logical-operators` — boolean operators
- :doc:`assignment` — storing expression results
