==
TP
==

Pulse timer. Generates a pulse of duration ``PT`` on the rising edge of
input ``IN``.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.2.3.3
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
   * - ``IN``
     - ``BOOL``
     - Trigger input
   * - ``PT``
     - ``TIME``
     - Pulse duration

Outputs
-------

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Name
     - Type
     - Description
   * - ``Q``
     - ``BOOL``
     - Pulse output (TRUE during pulse)
   * - ``ET``
     - ``TIME``
     - Elapsed time

Behavior
--------

When ``IN`` transitions from ``FALSE`` to ``TRUE`` (rising edge), the output
``Q`` becomes ``TRUE`` and the elapsed time ``ET`` begins counting from
``T#0s``. The output ``Q`` remains ``TRUE`` until ``ET`` reaches the preset
time ``PT``, at which point ``Q`` becomes ``FALSE``. Changes to ``IN`` during
the pulse have no effect; the pulse always runs for the full duration ``PT``.

Example
-------

.. code-block:: iec61131

   VAR
     pulse1 : TP;
     trigger : BOOL;
     output : BOOL;
   END_VAR

   pulse1(IN := trigger, PT := T#1s);
   output := pulse1.Q;

See Also
--------

- :doc:`ton` — on-delay timer
- :doc:`tof` — off-delay timer
