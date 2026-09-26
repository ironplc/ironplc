=============
DATE_AND_TIME
=============

Combined date and time of day value.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits (second resolution)
   * - **Default**
     - ``DT#1970-01-01-00:00:00``
   * - **Range**
     - ``DT#1970-01-01-00:00:00`` to ``DT#2106-02-07-06:28:15``
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

A date is stored as a count of seconds since 1970-01-01, so a literal
outside ``DT#1970-01-01-00:00:00`` to ``DT#2106-02-07-06:28:15`` is reported as
:doc:`P2038 </reference/compiler/problems/P2038>`.

Example
-------

.. playground-with-program::
   :vars: event : DATE_AND_TIME; deadline : DATE_AND_TIME; on_time : BOOL;

   event := DT#2024-06-15-10:30:00;
   deadline := DT#2024-06-15-12:00:00;
   on_time := event < deadline;  (* on_time = TRUE *)

Literals
--------

.. code-block::

   DT#2024-01-15-14:30:00
   DATE_AND_TIME#2024-12-31-23:59:59

See Also
--------

- :doc:`ldate-and-time` — 64-bit date and time (Edition 3)
- :doc:`date` — calendar date
- :doc:`time-of-day` — time of day
- :doc:`time` — duration
