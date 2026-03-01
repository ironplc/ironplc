====================
Arithmetic Operators
====================

Arithmetic operators perform mathematical computations on numeric values.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.1
   * - **Support**
     - Supported for integer types

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
``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``). For integer division,
the result is truncated toward zero. The ``MOD`` operator returns the
remainder of integer division.

The unary negation operator ``-`` has higher precedence than the binary
arithmetic operators. Exponentiation ``**`` has higher precedence than
multiplication, division, and modulo.

Example
-------

.. code-block::

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
