================
LDATE_AND_TIME
================

64-bit combined date and time of day value.

.. include:: ../../../../includes/requires-edition3.rst

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits (millisecond resolution)
   * - **Default**
     - ``LDT#0001-01-01-00:00:00``
   * - **IEC 61131-3**
     - Section 2.3.1 (Edition 3)
   * - **Support**
     - Supported (:doc:`Edition 3 </reference/language/edition-support>`)

Example
-------

.. playground-with-program::
   :vars: event : LDATE_AND_TIME; deadline : LDATE_AND_TIME; on_time : BOOL;

   event := LDT#2024-06-15-10:30:00;
   deadline := LDT#2024-06-15-12:00:00;
   on_time := event < deadline;  (* on_time = TRUE *)

Literals
--------

.. code-block::

   LDT#2024-01-15-14:30:00
   LDATE_AND_TIME#2024-12-31-23:59:59

See Also
--------

- :doc:`date-and-time` — 32-bit date and time
- :doc:`ldate` — 64-bit calendar date (Edition 3)
- :doc:`ltime-of-day` — 64-bit time of day (Edition 3)
- :doc:`ltime` — 64-bit duration (Edition 3)
