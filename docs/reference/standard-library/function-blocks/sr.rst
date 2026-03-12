==
SR
==

Set-dominant bistable function block. A flip-flop where the set input takes
priority: if both ``S1`` and ``R`` are ``TRUE``, the output ``Q1`` is
``TRUE``.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.2.3.1
   * - **Support**
     - Supported

Inputs
------

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Name
     - Type
     - Description
   * - ``S1``
     - ``BOOL``
     - Set input (dominant)
   * - ``R``
     - ``BOOL``
     - Reset input

Outputs
-------

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Name
     - Type
     - Description
   * - ``Q1``
     - ``BOOL``
     - Output state

Behavior
--------

The output is computed as ``Q1 := S1 OR (NOT R AND Q1)``. When ``S1`` is
``TRUE``, the output is set regardless of ``R``. When only ``R`` is ``TRUE``,
the output is cleared. The output retains its value between scans (latching).

Example
-------

This example shows that set dominates: both ``S1`` and ``R`` are ``TRUE``, yet
``output`` is ``TRUE``.

.. playground::

   PROGRAM main
      VAR
         latch : SR;
         output : BOOL;
      END_VAR

      latch(S1 := TRUE, R := TRUE, Q1 => output);
      (* output is TRUE because set dominates *)
   END_PROGRAM

See Also
--------

- :doc:`rs` — reset-dominant bistable
