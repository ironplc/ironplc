========
ADD_TIME
========

Returns the sum of two time durations.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │ADD_TIME ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION ADD_TIME : TIME
     VAR_INPUT
       IN1 : TIME;
       IN2 : TIME;
     END_VAR
   END_FUNCTION

Both inputs and the return value are ``TIME``.

Description
-----------

Returns the sum of two time durations *IN1* and *IN2*.
``ADD_TIME(a, b)`` adds two ``TIME`` values together.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := ADD_TIME(T#2s, T#3s);   (* result = T#5s *)

See Also
--------

* :doc:`sub_time` — subtract durations
* :doc:`mul_time` — scale duration
* :doc:`div_time` — divide duration

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: ADD (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_add.html>`_
* `Beckhoff TwinCAT 3: ADD (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/18014401038341259.html>`_
* `Fernhill SCADA: ADD_TIME <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-add.html>`_
