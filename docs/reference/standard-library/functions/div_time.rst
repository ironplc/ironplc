========
DIV_TIME
========

Divides a time duration by a numeric value.

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
     - ``ANY_NUM``
     - ``TIME``
     - Supported

Description
-----------

Returns *IN1* divided by the numeric value *IN2*. The result is
a ``TIME`` value. When *IN2* is a floating-point type (``REAL`` or
``LREAL``), the result is truncated to whole milliseconds.

Example
-------

.. playground-with-program::
   :vars: result : TIME;

   result := DIV_TIME(T#6s, 3);        (* result = T#2s *)

See Also
--------

- :doc:`mul_time` — scale duration
- :doc:`add_time` — add durations
- :doc:`sub_time` — subtract durations
