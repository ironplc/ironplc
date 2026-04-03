=========
DT_TO_TOD
=========

Extracts the time-of-day portion from a date-and-time value.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5
   * - **Support**
     - Supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 20 20 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``DATE_AND_TIME``
     - ``TIME_OF_DAY``
     - Supported

Description
-----------

Extracts the time-of-day portion from *IN*, returning a ``TIME_OF_DAY``
value in milliseconds since midnight.

The long-form alias ``DATE_AND_TIME_TO_TIME_OF_DAY`` is also supported.

Example
-------

.. playground-with-program::
   :vars: result : TIME_OF_DAY;

   result := DT_TO_TOD(DT#2000-01-01-12:00:00);

See Also
--------

- :doc:`dt_to_date` --- extract date from datetime
- :doc:`concat_date_tod` --- combine date and time-of-day
