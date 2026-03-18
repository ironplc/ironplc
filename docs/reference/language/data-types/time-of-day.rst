===========
TIME_OF_DAY
===========

Time of day value.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits (millisecond resolution)
   * - **Default**
     - ``TOD#00:00:00``
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Example
-------

.. playground-with-program::
   :vars: shift_start : TIME_OF_DAY; now : TIME_OF_DAY; started : BOOL;

   shift_start := TOD#08:00:00;
   now := TOD#09:30:00;
   started := now > shift_start;  (* started = TRUE *)

Literals
--------

.. code-block::

   TOD#14:30:00
   TIME_OF_DAY#08:00:00
   TOD#23:59:59.999

See Also
--------

- :doc:`ltime-of-day` — 64-bit time of day (Edition 3)
- :doc:`date` — calendar date
- :doc:`date-and-time` — combined date and time
- :doc:`time` — duration
