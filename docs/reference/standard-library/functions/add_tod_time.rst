============
ADD_TOD_TIME
============

Adds a duration to a time-of-day value.

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

Returns a new ``TIME_OF_DAY`` offset from *IN1* by the duration *IN2*.
Both values are in milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME_OF_DAY;

   result := ADD_TOD_TIME(TOD#12:00:00, T#1h);  (* result = TOD#13:00:00 *)

See Also
--------

- :doc:`sub_tod_time` — subtract duration from time-of-day
- :doc:`sub_tod_tod` — difference between two times-of-day
