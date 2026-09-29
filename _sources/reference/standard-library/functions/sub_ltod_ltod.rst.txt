=============
SUB_LTOD_LTOD
=============

Returns the difference between two long times-of-day as a duration.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌───────────────┐
       IN1 ─┤               │
            │ SUB_LTOD_LTOD ├─ OUT
       IN2 ─┤               │
            └───────────────┘

.. code-block:: text

   FUNCTION SUB_LTOD_LTOD : LTIME
     VAR_INPUT
       IN1 : LTIME_OF_DAY;
       IN2 : LTIME_OF_DAY;
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
     - ``LTIME_OF_DAY``
     - The minuend time-of-day.
   * - ``IN2``
     - ``LTIME_OF_DAY``
     - The subtrahend time-of-day.

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

Returns the difference *IN1* minus *IN2* as an ``LTIME`` duration. Both
inputs and the result are in milliseconds.

The short form is :doc:`SUB_TOD_TOD <sub_tod_tod>`. An operand of the
short type (``TIME_OF_DAY`` where ``LTIME_OF_DAY`` is expected) is
accepted and widened to 64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LTIME;

   result := SUB_LTOD_LTOD(LTOD#10:00:00, LTOD#08:30:00);   (* result = LTIME#1h30m *)

See Also
--------

* :doc:`sub_tod_tod` — the short form
* :doc:`add_ltod_ltime` — add a duration to a long time-of-day
* :doc:`sub_ltod_ltime` — subtract a duration from a long time-of-day

References
----------

* IEC 61131-3 Edition 3, Table 30
