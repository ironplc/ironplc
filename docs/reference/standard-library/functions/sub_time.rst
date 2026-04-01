========
SUB_TIME
========

Returns the difference of two time durations.

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
     - ``TIME``
     - ``TIME``
     - ``TIME``
     - Supported

Description
-----------

Returns the difference *IN1* minus *IN2* as a time duration.

Example
-------

.. code-block:: iecst

   result := SUB_TIME(T#5s, T#2s);   (* result = T#3s *)

See Also
--------

- :doc:`add_time` — add durations
- :doc:`mul_time` — scale duration
- :doc:`div_time` — divide duration
