=====
LDATE
=====

64-bit calendar date value.

.. note::

   LDATE is an IEC 61131-3:2013 (Edition 3) type. You must pass
   ``--std-iec-61131-3=2013`` to the compiler. See :doc:`/reference/compiler/ironplcc`
   for details.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits (day resolution)
   * - **Default**
     - ``LDATE#0001-01-01``
   * - **IEC 61131-3**
     - Section 2.3.1 (Edition 3)
   * - **Support**
     - Supported (requires ``--std-iec-61131-3=2013``)

Example
-------

.. playground-with-program::
   :vars: today : LDATE; launch : LDATE; is_after : BOOL;

   today := LDATE#2024-06-15;
   launch := LDATE#2024-01-01;
   is_after := today > launch;  (* is_after = TRUE *)

Literals
--------

.. code-block::

   LDATE#2024-01-15
   LDATE#2024-12-31

See Also
--------

- :doc:`date` — 32-bit calendar date
- :doc:`ltime-of-day` — 64-bit time of day (Edition 3)
- :doc:`ldate-and-time` — 64-bit date and time (Edition 3)
- :doc:`ltime` — 64-bit duration (Edition 3)
