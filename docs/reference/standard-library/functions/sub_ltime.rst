=========
SUB_LTIME
=========

Returns the difference between two long durations.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌───────────┐
       IN1 ─┤           │
            │ SUB_LTIME ├─ OUT
       IN2 ─┤           │
            └───────────┘

.. code-block:: text

   FUNCTION SUB_LTIME : LTIME
     VAR_INPUT
       IN1 : LTIME;
       IN2 : LTIME;
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
     - The duration to subtract from.
   * - ``IN2``
     - ``LTIME``
     - The duration to subtract.

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
     - IN1 minus IN2.

Description
-----------

Returns *IN1* minus *IN2*, both ``LTIME`` durations, in 64-bit
milliseconds.

The short form is :doc:`SUB_TIME <sub_time>`. An operand of the short
type (``TIME`` where ``LTIME`` is expected) is accepted and widened to
64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LTIME;

   result := SUB_LTIME(LTIME#2h, LTIME#30m);   (* result = LTIME#1h30m *)

See Also
--------

* :doc:`sub_time` — the short form
* :doc:`add_ltime` — add long durations
* :doc:`mul_ltime` — scale a long duration
* :doc:`div_ltime` — divide a long duration

References
----------

* IEC 61131-3 Edition 3, Table 30
