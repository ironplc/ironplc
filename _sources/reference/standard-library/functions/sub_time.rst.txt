========
SUB_TIME
========

Returns the difference of two time durations.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │SUB_TIME ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION SUB_TIME : TIME
     VAR_INPUT
       IN1 : TIME;
       IN2 : TIME;
     END_VAR
   END_FUNCTION

Both inputs and the return value are ``TIME``.

Description
-----------

Returns the difference *IN1* minus *IN2* as a time duration.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := SUB_TIME(T#5s, T#2s);   (* result = T#3s *)

See Also
--------

* :doc:`add_time` — add durations
* :doc:`mul_time` — scale duration
* :doc:`div_time` — divide duration

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: SUB (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sub.html>`_
* `Beckhoff TwinCAT 3: SUB (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528870027.html>`_
* `Fernhill SCADA: SUB_TIME <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-sub.html>`_
