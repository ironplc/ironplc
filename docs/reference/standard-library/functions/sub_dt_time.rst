==========
SUB_DT_TIME
==========

Subtracts a duration from a date-and-time value.

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

Returns a new ``DATE_AND_TIME`` offset from *IN1* by subtracting the
duration *IN2*. The duration is converted from milliseconds to seconds.

Example
-------

.. code-block:: iecst

   result := SUB_DT_TIME(DT#2000-01-01-01:00:00, T#1h);

See Also
--------

- :doc:`add_dt_time` — add duration to datetime
- :doc:`sub_dt_dt` — difference between two datetimes
