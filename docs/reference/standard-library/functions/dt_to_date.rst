==========
DT_TO_DATE
==========

Extracts the date portion from a date-and-time value.

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
     - ``DATE``
     - Supported

Description
-----------

Extracts the date portion from *IN* by stripping the time-of-day component,
returning a ``DATE`` value representing midnight of the same day.

The long-form alias ``DATE_AND_TIME_TO_DATE`` is also supported.

Example
-------

.. playground-with-program::
   :vars: result : DATE;

   result := DT_TO_DATE(DT#2000-01-01-12:00:00);

See Also
--------

- :doc:`dt_to_tod` --- extract time-of-day from datetime
- :doc:`concat_date_tod` --- combine date and time-of-day
