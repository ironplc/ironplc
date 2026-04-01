=========
SUB_DT_DT
=========

Returns the difference between two date-and-time values as a duration.

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
     - ``DATE_AND_TIME``
     - ``TIME``
     - Supported

Description
-----------

Returns the time difference *IN1* minus *IN2* as a ``TIME`` duration
in milliseconds. The internal subtraction is in seconds, then converted
to milliseconds.

Example
-------

.. code-block:: iecst

   result := SUB_DT_DT(DT#2000-01-01-01:00:00, DT#2000-01-01-00:00:00);
   (* result = T#1h *)

See Also
--------

- :doc:`add_dt_time` — add duration to datetime
- :doc:`sub_dt_time` — subtract duration from datetime
