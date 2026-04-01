==========
ADD_DT_TIME
==========

Adds a duration to a date-and-time value.

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
     - ``DATE_AND_TIME``
     - ``TIME``
     - ``DATE_AND_TIME``
     - Supported

Description
-----------

Returns a new ``DATE_AND_TIME`` offset from *IN1* by the duration *IN2*.
The duration is converted from milliseconds to seconds before being added.

Example
-------

.. code-block:: iecst

   result := ADD_DT_TIME(DT#2000-01-01-00:00:00, T#1h);

See Also
--------

- :doc:`sub_dt_time` — subtract duration from datetime
- :doc:`sub_dt_dt` — difference between two datetimes
