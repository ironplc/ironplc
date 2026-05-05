============
ADD_TOD_TIME
============

Adds a duration to a time-of-day value.

Signature
---------

.. code-block:: text

            ┌────────────┐
       IN1 ─┤            │
            │ADD_TOD_TIME├─ OUT
       IN2 ─┤            │
            └────────────┘

.. code-block:: text

   FUNCTION ADD_TOD_TIME : TIME_OF_DAY
     VAR_INPUT
       IN1 : TIME_OF_DAY;
       IN2 : TIME;
     END_VAR
   END_FUNCTION

The return type is ``TIME_OF_DAY``. *IN1* is ``TIME_OF_DAY`` and
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
     - ``TIME_OF_DAY``
     - The time-of-day to offset.
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
     - ``TIME_OF_DAY``
     - IN1 offset by IN2.

Description
-----------

Returns a new ``TIME_OF_DAY`` offset from *IN1* by the duration *IN2*.
Both values are in milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME_OF_DAY;

   result := ADD_TOD_TIME(TOD#12:00:00, T#1h);  (* result = TOD#13:00:00 *)

See Also
--------

* :doc:`sub_tod_time` — subtract duration from time-of-day
* :doc:`sub_tod_tod` — difference between two times-of-day

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: ADD (covers time arithmetic) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_add.html>`_
* `Beckhoff TwinCAT 3: ADD (covers time arithmetic) <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/18014401038341259.html>`_
* `Fernhill SCADA: ADD_TOD_TIME <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-add-tod.html>`_
