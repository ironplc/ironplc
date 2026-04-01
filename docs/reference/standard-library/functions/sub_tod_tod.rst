==========
SUB_TOD_TOD
==========

Returns the difference between two time-of-day values as a duration.

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
     - ``TIME_OF_DAY``
     - ``TIME``
     - Supported

Description
-----------

Returns the difference *IN1* minus *IN2* as a ``TIME`` duration.
Both inputs and the result are in milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := SUB_TOD_TOD(TOD#14:00:00, TOD#12:00:00);
   (* result = T#2h *)

See Also
--------

- :doc:`add_tod_time` — add duration to time-of-day
- :doc:`sub_tod_time` — subtract duration from time-of-day
