=========
DIV_LTIME
=========

Divides a long duration by a number.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌───────────┐
       IN1 ─┤           │
            │ DIV_LTIME ├─ OUT
       IN2 ─┤           │
            └───────────┘

.. code-block:: text

   FUNCTION DIV_LTIME : LTIME
     VAR_INPUT
       IN1 : LTIME;
       IN2 : ANY_NUM;
     END_VAR
   END_FUNCTION

The return type is ``LTIME``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``LTIME``
     - The duration to divide.
   * - ``IN2``
     - ``ANY_NUM``
     - The numeric divisor.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``LTIME``
     - IN1 divided by IN2.

Description
-----------

Returns *IN1* divided by the numeric value *IN2*. The result is an
``LTIME`` value. When *IN2* is a floating-point type (``REAL`` or
``LREAL``), the quotient is computed as ``LREAL`` and truncated to whole
milliseconds.

The short form is :doc:`DIV_TIME <div_time>`. An operand of the short
type (``TIME`` where ``LTIME`` is expected) is accepted and widened to
64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LTIME;

   result := DIV_LTIME(LTIME#2h, 3);   (* result = LTIME#40m *)

See Also
--------

* :doc:`div_time` — the short form
* :doc:`mul_ltime` — scale a long duration
* :doc:`add_ltime` — add long durations
* :doc:`sub_ltime` — subtract long durations

References
----------

* IEC 61131-3 Edition 3, Table 30
