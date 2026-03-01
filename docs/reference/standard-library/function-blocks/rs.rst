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
     - Not yet supported

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

The output ``Q1`` is set to ``TRUE`` when ``S`` is ``TRUE``, and reset to
``FALSE`` when ``R1`` is ``TRUE``. Because the reset input is dominant, if both
``S`` and ``R1`` are ``TRUE`` simultaneously, the output ``Q1`` is ``FALSE``.
The output retains its value between scans.

Example
-------

.. code-block:: iec61131

   VAR
     latch1 : RS;
     set_signal : BOOL;
     reset_signal : BOOL;
     output : BOOL;
   END_VAR

   latch1(S := set_signal, R1 := reset_signal);
   output := latch1.Q1;

See Also
--------

- :doc:`sr` — set-dominant bistable
