=================
Logical Operators
=================

Logical operators perform boolean logic on ``BOOL`` operands.

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
   * - ``AND``
     - ``a AND b``
     - Logical AND (also written ``&``)
   * - ``OR``
     - ``a OR b``
     - Logical OR
   * - ``XOR``
     - ``a XOR b``
     - Logical exclusive OR
   * - ``NOT``
     - ``NOT a``
     - Logical complement (unary)

Description
-----------

``AND`` (or ``&``) returns ``TRUE`` only when both operands are ``TRUE``.
``OR`` returns ``TRUE`` when at least one operand is ``TRUE``. ``XOR`` returns
``TRUE`` when exactly one operand is ``TRUE``. ``NOT`` inverts a single
boolean value.

Precedence from highest to lowest: ``NOT``, ``AND`` / ``&``, ``XOR``, ``OR``.
Use parentheses to override the default precedence.

Example
-------

.. code-block:: iec61131

   PROGRAM main
       VAR
           sensor_a : BOOL := TRUE;
           sensor_b : BOOL := FALSE;
           enable : BOOL := TRUE;
           run_motor : BOOL;
           alarm : BOOL;
       END_VAR

       run_motor := enable AND (sensor_a OR sensor_b);
       alarm := sensor_a XOR sensor_b;
   END_PROGRAM

See Also
--------

- :doc:`/reference/language/data-types/bool` — boolean data type
- :doc:`comparison-operators` — relational operators producing BOOL
- :doc:`arithmetic-operators` — numeric operators
