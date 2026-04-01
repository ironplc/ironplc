=============
SUB_DATE_DATE
=============

Returns the difference between two dates as a duration.

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
     - ``DATE``
     - ``TIME``
     - Supported

Description
-----------

Returns the difference *IN1* minus *IN2* as a ``TIME`` duration in
milliseconds. The internal subtraction is in seconds, then converted
to milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := SUB_DATE_DATE(D#2000-01-02, D#2000-01-01);
   (* result = T#24h *)

See Also
--------

- :doc:`sub_dt_dt` — difference between two datetimes
