=====
LTIME
=====

64-bit duration value representing an interval of time.

.. include:: ../../../../includes/requires-edition3.rst

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits (millisecond resolution)
   * - **Range**
     - about ±292 million years
   * - **Default**
     - ``LTIME#0s``
   * - **IEC 61131-3**
     - Section 2.3.1 (Edition 3)
   * - **Support**
     - Supported (:doc:`Edition 3 </reference/language/edition-support>`)

Example
-------

.. playground-with-program::
   :dialect: iec61131-3-ed3
   :vars: a : LTIME; b : LTIME; c : LTIME;

   a := LTIME#1h;
   b := LTIME#30m;
   c := a + b;  (* c = LTIME#1h30m *)

A duration is stored as a signed count of milliseconds. ``LTIME`` counts them
in 64 bits, so it reaches far past the ±24.8 days a 32-bit :doc:`time` holds.

An ``LTIME`` accepts a ``T#`` literal, which widens, but a ``TIME`` does not
accept an ``LTIME#`` one — that narrows, and is reported as
:doc:`P4022 </reference/compiler/problems/P4022>`.

Literals
--------

.. code-block::

   LTIME#100ms
   LTIME#5s
   LTIME#2h
   ltime#-500ms

Supported units: days (``d``), hours (``h``), minutes (``m``),
seconds (``s``), milliseconds (``ms``). Units are case-insensitive,
so ``LTIME#5S`` and ``LTIME#5s`` are equivalent. The prefix is likewise
case-insensitive.

See Also
--------

- :doc:`time` — 32-bit duration
- :doc:`/reference/standard-library/function-blocks/ton` — on-delay timer
- :doc:`/reference/standard-library/function-blocks/tof` — off-delay timer
- :doc:`/reference/standard-library/function-blocks/tp` — pulse timer
- :doc:`/reference/compiler/problems/P0010` — error when Edition 3 flag is missing
