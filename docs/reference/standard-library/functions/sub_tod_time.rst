============
SUB_TOD_TIME
============

Subtracts a duration from a time-of-day value.

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
     - ``TIME_OF_DAY``
     - ``TIME``
     - ``TIME_OF_DAY``
     - Supported

Description
-----------

Returns a new ``TIME_OF_DAY`` offset from *IN1* by subtracting the
duration *IN2*. Both values are in milliseconds.

Example
-------

.. code-block:: iecst

   result := SUB_TOD_TIME(TOD#14:00:00, T#1h);  (* result = TOD#13:00:00 *)

See Also
--------

- :doc:`add_tod_time` — add duration to time-of-day
- :doc:`sub_tod_tod` — difference between two times-of-day
