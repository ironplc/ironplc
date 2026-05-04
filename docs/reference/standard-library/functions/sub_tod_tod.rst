===========
SUB_TOD_TOD
===========

Returns the difference between two time-of-day values as a duration.

Signature
---------

.. code-block:: text

            ┌───────────┐
       IN1 ─┤           │
            │SUB_TOD_TOD├─ OUT
       IN2 ─┤           │
            └───────────┘

.. code-block:: text

   FUNCTION SUB_TOD_TOD : TIME
     VAR_INPUT
       IN1 : TIME_OF_DAY;
       IN2 : TIME_OF_DAY;
     END_VAR
   END_FUNCTION

The return type is ``TIME``. Both inputs are ``TIME_OF_DAY``.

Description
-----------

Returns the difference *IN1* minus *IN2* as a ``TIME`` duration.
Both inputs and the result are in milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := SUB_TOD_TOD(TOD#14:00:00, TOD#12:00:00);
   (* result = T#2h *)

See Also
--------

* :doc:`add_tod_time` — add duration to time-of-day
* :doc:`sub_tod_time` — subtract duration from time-of-day

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: SUB (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sub.html>`_
* `Beckhoff TwinCAT 3: SUB (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528870027.html>`_
* `Fernhill SCADA: SUB_TOD_TOD <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-sub-tod-tod.html>`_
