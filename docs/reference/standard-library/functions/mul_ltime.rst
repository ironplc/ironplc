=========
MUL_LTIME
=========

Multiplies a long duration by a number.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌───────────┐
       IN1 ─┤           │
            │ MUL_LTIME ├─ OUT
       IN2 ─┤           │
            └───────────┘

.. code-block:: text

   FUNCTION MUL_LTIME : LTIME
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
     - The duration to scale.
   * - ``IN2``
     - ``ANY_NUM``
     - The numeric factor to multiply by.

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
     - IN1 multiplied by IN2.

Description
-----------

Returns *IN1* multiplied by the numeric value *IN2*. The result is an
``LTIME`` value. When *IN2* is a floating-point type (``REAL`` or
``LREAL``), the product is computed as ``LREAL``, so that no millisecond
count is rounded to fit a ``REAL`` mantissa, and is truncated to whole
milliseconds.

The short form is :doc:`MUL_TIME <mul_time>`. An operand of the short
type (``TIME`` where ``LTIME`` is expected) is accepted and widened to
64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LTIME;

   result := MUL_LTIME(LTIME#30d, 3);   (* result = LTIME#90d *)

See Also
--------

* :doc:`mul_time` — the short form
* :doc:`div_ltime` — divide a long duration
* :doc:`add_ltime` — add long durations
* :doc:`sub_ltime` — subtract long durations

References
----------

* IEC 61131-3 Edition 3, Table 30
