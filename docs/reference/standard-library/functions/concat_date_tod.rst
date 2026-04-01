==============
CONCAT_DATE_TOD
==============

Combines a date and a time-of-day into a date-and-time value.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.8
   * - **Support**
     - Supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 20 20 20 30

   * - #
     - Input (IN1)
     - Input (IN2)
     - Return Type
     - Support
   * - 1
     - ``DATE``
     - ``TIME_OF_DAY``
     - ``DATE_AND_TIME``
     - Supported

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

- :doc:`add_dt_time` — add duration to datetime
- :doc:`sub_dt_dt` — difference between two datetimes
