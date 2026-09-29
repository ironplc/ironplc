==============
SUB_LTOD_LTIME
==============

Subtracts a long duration from a long time-of-day.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌────────────────┐
       IN1 ─┤                │
            │ SUB_LTOD_LTIME ├─ OUT
       IN2 ─┤                │
            └────────────────┘

.. code-block:: text

   FUNCTION SUB_LTOD_LTIME : LTIME_OF_DAY
     VAR_INPUT
       IN1 : LTIME_OF_DAY;
       IN2 : LTIME;
     END_VAR
   END_FUNCTION

The return type is ``LTIME_OF_DAY``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``LTIME_OF_DAY``
     - The time-of-day to offset.
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
     - ``LTIME_OF_DAY``
     - IN1 minus IN2.

Description
-----------

Returns a new ``LTIME_OF_DAY`` that is *IN1* moved back by the duration
*IN2*. Both values are in milliseconds.

The short form is :doc:`SUB_TOD_TIME <sub_tod_time>`. An operand of the
short type (``TIME_OF_DAY`` and ``TIME`` where ``LTIME_OF_DAY`` and
``LTIME`` is expected) is accepted and widened to 64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LTIME_OF_DAY;

   result := SUB_LTOD_LTIME(LTOD#10:00:00, LTIME#30m);   (* result = LTOD#09:30:00 *)

See Also
--------

* :doc:`sub_tod_time` — the short form
* :doc:`add_ltod_ltime` — add a duration to a long time-of-day
* :doc:`sub_ltod_ltod` — difference between two long times-of-day

References
----------

* IEC 61131-3 Edition 3, Table 30
