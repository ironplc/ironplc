===========
SUB_DT_TIME
===========

Subtracts a duration from a date-and-time value.

Signature
---------

.. code-block:: text

            ┌──────────┐
       IN1 ─┤          │
            │SUB_DT_TIME├─ OUT
       IN2 ─┤          │
            └──────────┘

.. code-block:: text

   FUNCTION SUB_DT_TIME : DATE_AND_TIME
     VAR_INPUT
       IN1 : DATE_AND_TIME;
       IN2 : TIME;
     END_VAR
   END_FUNCTION

The return type is ``DATE_AND_TIME``. *IN1* is ``DATE_AND_TIME`` and
*IN2* is ``TIME``.

Description
-----------

Returns a new ``DATE_AND_TIME`` offset from *IN1* by subtracting the
duration *IN2*. The duration is converted from milliseconds to seconds.

Example
-------

.. playground-with-program::
   :vars: result : DATE_AND_TIME;

   result := SUB_DT_TIME(DT#2000-01-01-01:00:00, T#1h);

See Also
--------

* :doc:`add_dt_time` — add duration to datetime
* :doc:`sub_dt_dt` — difference between two datetimes

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: SUB (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sub.html>`_
* `Beckhoff TwinCAT 3: SUB (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528870027.html>`_
* `Fernhill SCADA: SUB_DT_TIME <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-sub-dt-time.html>`_
