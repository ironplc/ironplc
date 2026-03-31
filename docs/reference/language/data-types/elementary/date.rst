====
DATE
====

Calendar date value.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 32 bits (day resolution)
   * - **Default**
     - ``D#0001-01-01``
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Example
-------

.. playground-with-program::
   :vars: today : DATE; launch : DATE; is_after : BOOL;

   today := D#2024-06-15;
   launch := D#2024-01-01;
   is_after := today > launch;  (* is_after = TRUE *)

Literals
--------

.. code-block::

   D#2024-01-15
   DATE#2024-12-31

See Also
--------

- :doc:`ldate` — 64-bit calendar date (Edition 3)
- :doc:`time-of-day` — time of day
- :doc:`date-and-time` — combined date and time
- :doc:`time` — duration
