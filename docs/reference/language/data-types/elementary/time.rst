====
TIME
====

Duration value representing an interval of time.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits (millisecond resolution)
   * - **Default**
     - ``T#0s``
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Example
-------

.. playground-with-program::
   :vars: duration : TIME;

   duration := T#2s;

Literals
--------

.. code-block::

   T#100ms
   T#2s
   T#-500ms
   TIME#5S

Supported units: days (``d``), hours (``h``), minutes (``m``),
seconds (``s``), milliseconds (``ms``). Units are case-insensitive,
so ``T#5S`` and ``T#5s`` are equivalent. The prefix ``T#`` (or
``TIME#``) is likewise case-insensitive.

See Also
--------

- :doc:`ltime` — 64-bit duration (Edition 3)
- :doc:`/reference/standard-library/function-blocks/ton` — on-delay timer
- :doc:`/reference/standard-library/function-blocks/tof` — off-delay timer
- :doc:`/reference/standard-library/function-blocks/tp` — pulse timer
- :doc:`date` — calendar date
- :doc:`time-of-day` — time of day
