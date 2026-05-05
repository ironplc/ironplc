===============
CONCAT_DATE_TOD
===============

Combines a date and a time-of-day into a date-and-time value.

Signature
---------

.. code-block:: text

            ┌───────────────┐
       IN1 ─┤               │
            │CONCAT_DATE_TOD├─ OUT
       IN2 ─┤               │
            └───────────────┘

.. code-block:: text

   FUNCTION CONCAT_DATE_TOD : DATE_AND_TIME
     VAR_INPUT
       IN1 : DATE;
       IN2 : TIME_OF_DAY;
     END_VAR
   END_FUNCTION

The return type is ``DATE_AND_TIME``. *IN1* is ``DATE`` and *IN2* is
``TIME_OF_DAY``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN1``
     - ``DATE``
     - The date component.
   * - ``IN2``
     - ``TIME_OF_DAY``
     - The time-of-day component.

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
     - The combined date and time-of-day.

Description
-----------

Combines the date portion from *IN1* with the time-of-day portion from
*IN2* to produce a ``DATE_AND_TIME`` value. The time-of-day value is
converted from milliseconds to seconds before being added to the date.

.. note::

   Sub-second precision from the ``TIME_OF_DAY`` input is lost because
   ``DATE_AND_TIME`` is stored in whole seconds.

Example
-------

.. playground-with-program::
   :vars: result : DATE_AND_TIME;

   result := CONCAT_DATE_TOD(D#2000-01-01, TOD#12:00:00);

See Also
--------

* :doc:`add_dt_time` — add duration to datetime
* :doc:`sub_dt_dt` — difference between two datetimes

References
----------

* IEC 61131-3 §2.5.1.5.8
* `CODESYS: CONCAT (covers date/time concatenation) <https://content.helpme-codesys.com/en/libs/Standard/Current/String-Functions/CONCAT.html>`_
* `Fernhill SCADA: CONCAT_DATE_TOD <https://www.fernhillsoftware.com/help/iec-61131/common-elements/date-time-functions/time-concat-date-tod.html>`_

.. Beckhoff TwinCAT 3 does not have a dedicated CONCAT_DATE_TOD page.
