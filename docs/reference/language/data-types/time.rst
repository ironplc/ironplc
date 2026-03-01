====
TIME
====

Duration value representing an interval of time.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits (microsecond resolution)
   * - **Default**
     - ``T#0s``
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Not yet supported

Literals
--------

.. code-block::

   T#100ms
   T#2s
   T#1h30m
   T#5d12h30m15s
   TIME#500us

Components can be combined: days (``d``), hours (``h``), minutes (``m``),
seconds (``s``), milliseconds (``ms``), microseconds (``us``).

See Also
--------

- :doc:`/reference/standard-library/function-blocks/ton` — on-delay timer
- :doc:`/reference/standard-library/function-blocks/tof` — off-delay timer
- :doc:`/reference/standard-library/function-blocks/tp` — pulse timer
- :doc:`date` — calendar date
- :doc:`time-of-day` — time of day
