=============
SUB_LDT_LTIME
=============

Subtracts a long duration from a long date-and-time.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌───────────────┐
       IN1 ─┤               │
            │ SUB_LDT_LTIME ├─ OUT
       IN2 ─┤               │
            └───────────────┘

.. code-block:: text

   FUNCTION SUB_LDT_LTIME : LDATE_AND_TIME
     VAR_INPUT
       IN1 : LDATE_AND_TIME;
       IN2 : LTIME;
     END_VAR
   END_FUNCTION

The return type is ``LDATE_AND_TIME``.

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
     - The date-and-time to offset.
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
     - ``LDATE_AND_TIME``
     - IN1 minus IN2.

Description
-----------

Returns a new ``LDATE_AND_TIME`` that is *IN1* moved back by the
duration *IN2*. The duration is converted from milliseconds to seconds
before being subtracted.

The short form is :doc:`SUB_DT_TIME <sub_dt_time>`. An operand of the
short type (``DATE_AND_TIME`` and ``TIME`` where ``LDATE_AND_TIME`` and
``LTIME`` is expected) is accepted and widened to 64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LDATE_AND_TIME;

   result := SUB_LDT_LTIME(LDT#2000-01-01-01:00:00, LTIME#1h);
   (* result = LDT#2000-01-01-00:00:00 *)

See Also
--------

* :doc:`sub_dt_time` — the short form
* :doc:`add_ldt_ltime` — add a duration to a long date-and-time
* :doc:`sub_ldt_ldt` — difference between two long date-and-times

References
----------

* IEC 61131-3 Edition 3, Table 30
