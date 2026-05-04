============
SUB_TOD_TIME
============

Subtracts a duration from a time-of-day value.

Signature
---------

.. code-block:: text

            ┌────────────┐
       IN1 ─┤            │
            │SUB_TOD_TIME├─ OUT
       IN2 ─┤            │
            └────────────┘

.. code-block:: text

   FUNCTION SUB_TOD_TIME : TIME_OF_DAY
     VAR_INPUT
       IN1 : TIME_OF_DAY;
       IN2 : TIME;
     END_VAR
   END_FUNCTION

The return type is ``TIME_OF_DAY``. *IN1* is ``TIME_OF_DAY`` and
*IN2* is ``TIME``.

Description
-----------

Returns a new ``TIME_OF_DAY`` offset from *IN1* by subtracting the
duration *IN2*. Both values are in milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME_OF_DAY;

   result := SUB_TOD_TIME(TOD#14:00:00, T#1h);  (* result = TOD#13:00:00 *)

See Also
--------

* :doc:`add_tod_time` — add duration to time-of-day
* :doc:`sub_tod_tod` — difference between two times-of-day

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: SUB (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sub.html>`_
* `Beckhoff TwinCAT 3: SUB (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528870027.html>`_
* `Fernhill SCADA: SUB_TOD_TIME <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-sub-tod-time.html>`_
