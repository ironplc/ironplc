===============
SUB_LDATE_LDATE
===============

Returns the difference between two long dates as a duration.

.. include:: ../../../includes/requires-edition3.rst

Signature
---------

.. code-block:: text

            ┌─────────────────┐
       IN1 ─┤                 │
            │ SUB_LDATE_LDATE ├─ OUT
       IN2 ─┤                 │
            └─────────────────┘

.. code-block:: text

   FUNCTION SUB_LDATE_LDATE : LTIME
     VAR_INPUT
       IN1 : LDATE;
       IN2 : LDATE;
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
     - ``LDATE``
     - The minuend date.
   * - ``IN2``
     - ``LDATE``
     - The subtrahend date.

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

The short form is :doc:`SUB_DATE_DATE <sub_date_date>`. An operand of
the short type (``DATE`` where ``LDATE`` is expected) is accepted and
widened to 64 bits.

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: result : LTIME;

   result := SUB_LDATE_LDATE(LDATE#2000-01-02, LDATE#2000-01-01);
   (* result = LTIME#24h *)

See Also
--------

* :doc:`sub_date_date` — the short form
* :doc:`sub_ldt_ldt` — difference between two long date-and-times

References
----------

* IEC 61131-3 Edition 3, Table 30
