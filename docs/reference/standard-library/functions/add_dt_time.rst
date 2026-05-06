===========
ADD_DT_TIME
===========

Adds a duration to a date-and-time value.

Signature
---------

.. code-block:: text

            ┌──────────┐
       IN1 ─┤          │
            │ADD_DT_TIME├─ OUT
       IN2 ─┤          │
            └──────────┘

.. code-block:: text

   FUNCTION ADD_DT_TIME : DATE_AND_TIME
     VAR_INPUT
       IN1 : DATE_AND_TIME;
       IN2 : TIME;
     END_VAR
   END_FUNCTION

The return type is ``DATE_AND_TIME``. *IN1* is ``DATE_AND_TIME`` and
*IN2* is ``TIME``.

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
     - The date-and-time to offset.
   * - ``IN2``
     - ``TIME``
     - The duration to add.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``DATE_AND_TIME``
     - IN1 offset by IN2.

Description
-----------

Returns a new ``DATE_AND_TIME`` offset from *IN1* by the duration *IN2*.
The duration is converted from milliseconds to seconds before being added.

Example
-------

.. playground-with-program::
   :vars: result : DATE_AND_TIME;

   result := ADD_DT_TIME(DT#2000-01-01-00:00:00, T#1h);

See Also
--------

* :doc:`sub_dt_time` — subtract duration from datetime
* :doc:`sub_dt_dt` — difference between two datetimes

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: ADD (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_add.html>`_
* `Beckhoff TwinCAT 3: ADD (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/18014401038341259.html>`_
* `Fernhill SCADA: ADD_DT_TIME <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-add-dt.html>`_
