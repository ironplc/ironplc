========
ADD_TIME
========

Returns the sum of two time durations.

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

Returns the sum of two time durations *IN1* and *IN2*.
``ADD_TIME(a, b)`` adds two ``TIME`` values together.

Example
-------

.. code-block:: iecst

   result := ADD_TIME(T#2s, T#3s);   (* result = T#5s *)

See Also
--------

- :doc:`sub_time` — subtract durations
- :doc:`mul_time` — scale duration
- :doc:`div_time` — divide duration
