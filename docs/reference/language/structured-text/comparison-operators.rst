====================
Comparison Operators
====================

Comparison operators compare two values and produce a ``BOOL`` result.

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
   * - ``=``
     - ``a = b``
     - Equal to
   * - ``<>``
     - ``a <> b``
     - Not equal to
   * - ``<``
     - ``a < b``
     - Less than
   * - ``>``
     - ``a > b``
     - Greater than
   * - ``<=``
     - ``a <= b``
     - Less than or equal to
   * - ``>=``
     - ``a >= b``
     - Greater than or equal to

Description
-----------

Comparison operators compare two operands of the same type and return a
``BOOL`` value. They apply to integer types (``SINT``, ``INT``, ``DINT``,
``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``).

Equality (``=``, ``<>``) has lower precedence than the relational operators
(``<``, ``>``, ``<=``, ``>=``). Both groups have lower precedence than
arithmetic operators.

Example
-------

.. code-block::

   PROGRAM main
       VAR
           temperature : INT := 75;
           setpoint : INT := 70;
           overheat : BOOL;
           at_target : BOOL;
       END_VAR

       overheat := temperature > setpoint;
       at_target := temperature = setpoint;
   END_PROGRAM

See Also
--------

- :doc:`arithmetic-operators` — numeric operators
- :doc:`logical-operators` — boolean operators
- :doc:`if` — conditional branching using boolean expressions
