========
MUL_TIME
========

Scales a time duration by a numeric factor.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │MUL_TIME ├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION MUL_TIME : TIME
     VAR_INPUT
       IN1 : TIME;
       IN2 : ANY_NUM;
     END_VAR
   END_FUNCTION

The return type is ``TIME``. *IN2* may be any numeric type.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``TIME``
     - The duration to scale.
   * - ``IN2``
     - ``ANY_NUM``
     - The numeric factor to multiply by.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``TIME``
     - IN1 multiplied by IN2.

Description
-----------

Returns *IN1* multiplied by the numeric value *IN2*. The result is
a ``TIME`` value. When *IN2* is a floating-point type (``REAL`` or
``LREAL``), the result is truncated to whole milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := MUL_TIME(T#2s, 3);        (* result = T#6s *)

See Also
--------

* :doc:`div_time` — divide duration
* :doc:`add_time` — add durations
* :doc:`sub_time` — subtract durations

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: MUL (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_mul.html>`_
* `Beckhoff TwinCAT 3: MUL (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528864651.html>`_
* `Fernhill SCADA: MULTIME <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-mul.html>`_
