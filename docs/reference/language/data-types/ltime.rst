=====
LTIME
=====

64-bit duration value representing an interval of time.

.. include:: ../../../includes/requires-edition3.rst

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits (millisecond resolution)
   * - **Default**
     - ``LTIME#0s``
   * - **IEC 61131-3**
     - Section 2.3.1 (Edition 3)
   * - **Support**
     - Supported (:doc:`Edition 3 </reference/language/edition-support>`)

Example
-------

.. playground-with-program::
   :vars: a : LTIME; b : LTIME; c : LTIME;

   a := LTIME#1h;
   b := LTIME#30m;
   c := a + b;  (* c = LTIME#1h30m *)

Literals
--------

.. code-block::

   LTIME#100ms
   LTIME#5s
   LTIME#1h30m
   LTIME#500us

Components can be combined: days (``d``), hours (``h``), minutes (``m``),
seconds (``s``), milliseconds (``ms``), microseconds (``us``).

See Also
--------

- :doc:`time` — 32-bit duration
- :doc:`/reference/standard-library/function-blocks/ton` — on-delay timer
- :doc:`/reference/standard-library/function-blocks/tof` — off-delay timer
- :doc:`/reference/standard-library/function-blocks/tp` — pulse timer
- :doc:`/reference/compiler/problems/P0010` — error when Edition 3 flag is missing
