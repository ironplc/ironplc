==
RS
==

Reset-dominant bistable function block. A flip-flop where the reset input takes
priority: if both ``S`` and ``R1`` are ``TRUE``, the output ``Q1`` is
``FALSE``.

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
   * - ``S``
     - ``BOOL``
     - Set input
   * - ``R1``
     - ``BOOL``
     - Reset input (dominant)

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

The output is computed as ``Q1 := NOT R1 AND (S OR Q1)``. When ``R1`` is
``TRUE``, the output is cleared regardless of ``S``. When only ``S`` is
``TRUE``, the output is set. The output retains its value between scans
(latching).

Example
-------

This example shows that reset dominates: both ``S`` and ``R1`` are ``TRUE``,
yet ``output`` is ``FALSE``.

.. playground::

   PROGRAM main
      VAR
         latch : RS;
         output : BOOL;
      END_VAR

      latch(S := TRUE, R1 := TRUE, Q1 => output);
      (* output is FALSE because reset dominates *)
   END_PROGRAM

See Also
--------

- :doc:`sr` — set-dominant bistable
