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
     - Not yet supported

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

The output ``Q1`` is set to ``TRUE`` when ``S1`` is ``TRUE``, and reset to
``FALSE`` when ``R`` is ``TRUE``. Because the set input is dominant, if both
``S1`` and ``R`` are ``TRUE`` simultaneously, the output ``Q1`` is ``TRUE``.
The output retains its value between scans.

Example
-------

.. code-block::

   VAR
     latch1 : SR;
     set_signal : BOOL;
     reset_signal : BOOL;
     output : BOOL;
   END_VAR

   latch1(S1 := set_signal, R := reset_signal);
   output := latch1.Q1;

See Also
--------

- :doc:`rs` — reset-dominant bistable
