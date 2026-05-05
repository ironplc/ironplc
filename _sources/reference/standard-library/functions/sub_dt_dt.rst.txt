=========
SUB_DT_DT
=========

Returns the difference between two date-and-time values as a duration.

Signature
---------

.. code-block:: text

            ┌─────────┐
       IN1 ─┤         │
            │SUB_DT_DT├─ OUT
       IN2 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION SUB_DT_DT : TIME
     VAR_INPUT
       IN1 : DATE_AND_TIME;
       IN2 : DATE_AND_TIME;
     END_VAR
   END_FUNCTION

The return type is ``TIME``. Both inputs are ``DATE_AND_TIME``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``DATE_AND_TIME``
     - The minuend date-and-time.
   * - ``IN2``
     - ``DATE_AND_TIME``
     - The subtrahend date-and-time.

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
     - IN1 minus IN2 as a duration.

Description
-----------

Returns the time difference *IN1* minus *IN2* as a ``TIME`` duration
in milliseconds. The internal subtraction is in seconds, then converted
to milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := SUB_DT_DT(DT#2000-01-01-01:00:00, DT#2000-01-01-00:00:00);
   (* result = T#1h *)

See Also
--------

* :doc:`add_dt_time` — add duration to datetime
* :doc:`sub_dt_time` — subtract duration from datetime

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: SUB (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sub.html>`_
* `Beckhoff TwinCAT 3: SUB (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528870027.html>`_
* `Fernhill SCADA: SUB_DT_DT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-sub-dt-dt.html>`_
