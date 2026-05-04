=============
SUB_DATE_DATE
=============

Returns the difference between two dates as a duration.

Signature
---------

.. code-block:: text

            ┌──────────────┐
       IN1 ─┤              │
            │ SUB_DATE_DATE├─ OUT
       IN2 ─┤              │
            └──────────────┘

.. code-block:: text

   FUNCTION SUB_DATE_DATE : TIME
     VAR_INPUT
       IN1 : DATE;
       IN2 : DATE;
     END_VAR
   END_FUNCTION

The return type is ``TIME``. Both inputs are ``DATE``.

Description
-----------

Returns the difference *IN1* minus *IN2* as a ``TIME`` duration in
milliseconds. The internal subtraction is in seconds, then converted
to milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := SUB_DATE_DATE(D#2000-01-02, D#2000-01-01);
   (* result = T#24h *)

See Also
--------

* :doc:`sub_dt_dt` — difference between two datetimes

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: SUB (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sub.html>`_
* `Beckhoff TwinCAT 3: SUB (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528870027.html>`_
* `Fernhill SCADA: SUB_DATE_DATE <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-sub-date.html>`_
