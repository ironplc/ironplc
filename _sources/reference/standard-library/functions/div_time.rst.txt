========
DIV_TIME
========

Divides a time duration by a numeric value.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │DIV_TIME ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION DIV_TIME : TIME
     VAR_INPUT
       IN1 : TIME;
       IN2 : ANY_NUM;
     END_VAR
   END_FUNCTION

The return type is ``TIME``. *IN2* may be any numeric type.

Description
-----------

Returns *IN1* divided by the numeric value *IN2*. The result is
a ``TIME`` value. When *IN2* is a floating-point type (``REAL`` or
``LREAL``), the result is truncated to whole milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := DIV_TIME(T#6s, 3);        (* result = T#2s *)

See Also
--------

* :doc:`mul_time` — scale duration
* :doc:`add_time` — add durations
* :doc:`sub_time` — subtract durations

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: DIV (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_div.html>`_
* `Beckhoff TwinCAT 3: DIV (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528875403.html>`_
* `Fernhill SCADA: DIVTIME <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-div.html>`_
