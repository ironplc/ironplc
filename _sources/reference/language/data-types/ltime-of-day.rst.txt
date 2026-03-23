==============
LTIME_OF_DAY
==============

64-bit time of day value.

.. include:: ../../../includes/requires-edition3.rst

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits (millisecond resolution)
   * - **Default**
     - ``LTOD#00:00:00``
   * - **IEC 61131-3**
     - Section 2.3.1 (Edition 3)
   * - **Support**
     - Supported (:doc:`Edition 3 </reference/language/edition-support>`)

Example
-------

.. playground-with-program::
   :vars: shift_start : LTIME_OF_DAY; now : LTIME_OF_DAY; started : BOOL;

   shift_start := LTOD#08:00:00;
   now := LTOD#09:30:00;
   started := now > shift_start;  (* started = TRUE *)

Literals
--------

.. code-block::

   LTOD#14:30:00
   LTIME_OF_DAY#08:00:00
   LTOD#23:59:59.999

See Also
--------

- :doc:`time-of-day` — 32-bit time of day
- :doc:`ldate` — 64-bit calendar date (Edition 3)
- :doc:`ldate-and-time` — 64-bit date and time (Edition 3)
- :doc:`ltime` — 64-bit duration (Edition 3)
