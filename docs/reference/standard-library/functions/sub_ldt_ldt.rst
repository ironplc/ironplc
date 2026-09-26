===========
SUB_LDT_LDT
===========

Returns the difference between two long date-and-times as a duration.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌─────────────┐
       IN1 ─┤             │
            │ SUB_LDT_LDT ├─ OUT
       IN2 ─┤             │
            └─────────────┘

.. code-block:: text

   FUNCTION SUB_LDT_LDT : LTIME
     VAR_INPUT
       IN1 : LDATE_AND_TIME;
       IN2 : LDATE_AND_TIME;
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
     - ``LDATE_AND_TIME``
     - The minuend date-and-time.
   * - ``IN2``
     - ``LDATE_AND_TIME``
     - The subtrahend date-and-time.

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
     - IN1 minus IN2 as a duration.

Description
-----------

Returns the difference *IN1* minus *IN2* as an ``LTIME`` duration in
milliseconds. The internal subtraction is in seconds, then converted to
milliseconds.

The short form is :doc:`SUB_DT_DT <sub_dt_dt>`. An operand of the short
type (``DATE_AND_TIME`` where ``LDATE_AND_TIME`` is expected) is
accepted and widened to 64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LTIME;

   result := SUB_LDT_LDT(LDT#2000-01-01-01:00:00, LDT#2000-01-01-00:00:00);
   (* result = LTIME#1h *)

See Also
--------

* :doc:`sub_dt_dt` — the short form
* :doc:`sub_ldate_ldate` — difference between two long dates
* :doc:`add_ldt_ltime` — add a duration to a long date-and-time

References
----------

* IEC 61131-3 Edition 3, Table 30
