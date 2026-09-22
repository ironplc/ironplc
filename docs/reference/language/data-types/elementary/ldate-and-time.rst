================
LDATE_AND_TIME
================

64-bit combined date and time of day value.

.. include:: ../../../../includes/requires-edition3.rst

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits (second resolution)
   * - **Default**
     - ``LDT#1970-01-01-00:00:00``
   * - **Range**
     - ``LDT#1970-01-01-00:00:00`` to ``LDT#2106-02-07-06:28:15``
   * - **IEC 61131-3**
     - Section 2.3.1 (Edition 3)
   * - **Support**
     - Supported (:doc:`Edition 3 </reference/language/edition-support>`)

A date is stored as a count of seconds since 1970-01-01, so a literal
outside ``LDT#1970-01-01-00:00:00`` to ``LDT#2106-02-07-06:28:15`` is reported as
:doc:`P2038 </reference/compiler/problems/P2038>`.

Literals
--------

.. code-block::

   LDT#2024-01-15-14:30:00
   LDATE_AND_TIME#2024-12-31-23:59:59

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: event : LDATE_AND_TIME; deadline : LDATE_AND_TIME; on_time : BOOL;

   event := LDT#2024-06-15-10:30:00;
   deadline := LDT#2024-06-15-12:00:00;
   on_time := event < deadline;  (* on_time = TRUE *)

See Also
--------

- :doc:`date-and-time` — 32-bit date and time
- :doc:`ldate` — 64-bit calendar date (Edition 3)
- :doc:`ltime-of-day` — 64-bit time of day (Edition 3)
- :doc:`ltime` — 64-bit duration (Edition 3)
