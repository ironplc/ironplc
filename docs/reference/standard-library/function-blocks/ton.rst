===
TON
===

On-delay timer. Output ``Q`` becomes ``TRUE`` after input ``IN`` has been
``TRUE`` for at least the preset time ``PT``.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.2.3.3
   * - **Support**
     - Supported

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``BOOL``
     - Starts the timer when TRUE. Resets the timer when FALSE.
   * - ``PT``
     - ``TIME``
     - Preset time duration. The timer runs for this duration after IN goes TRUE.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``Q``
     - ``BOOL``
     - TRUE when the elapsed time ET has reached the preset time PT.
   * - ``ET``
     - ``TIME``
     - Elapsed time since IN last went TRUE. Resets to zero when IN goes FALSE.

Behavior
--------

When ``IN`` transitions to ``TRUE``, the elapsed time ``ET`` begins counting
from ``T#0s``. When ``ET`` reaches the preset time ``PT``, the output ``Q``
becomes ``TRUE``. If ``IN`` returns to ``FALSE`` before ``ET`` reaches ``PT``,
both ``ET`` and ``Q`` are reset.

Example
-------

.. playground::

   PROGRAM main
      VAR
         myTimer : TON;
         start : BOOL := TRUE;
         done : BOOL;
         elapsed : TIME;
      END_VAR

      myTimer(IN := start, PT := T#5s, Q => done, ET => elapsed);
   END_PROGRAM

See Also
--------

- :doc:`tof` — off-delay timer
- :doc:`tp` — pulse timer
