===
TOF
===

Off-delay timer. Output ``Q`` stays ``TRUE`` for the preset time ``PT`` after
input ``IN`` goes ``FALSE``.

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
     - Holds the timer reset while TRUE. The off-delay starts when IN goes FALSE.
   * - ``PT``
     - ``TIME``
     - Preset time duration. The timer runs for this duration after IN goes FALSE.

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
     - TRUE while IN is TRUE and remains TRUE during the off-delay until ET reaches PT.
   * - ``ET``
     - ``TIME``
     - Elapsed time since IN last went FALSE. Resets to zero when IN goes TRUE.

Behavior
--------

When ``IN`` is ``TRUE``, the output ``Q`` is ``TRUE`` and the elapsed time
``ET`` is ``T#0s``. When ``IN`` transitions to ``FALSE``, ``ET`` begins
counting from ``T#0s``. The output ``Q`` remains ``TRUE`` until ``ET`` reaches
the preset time ``PT``, at which point ``Q`` becomes ``FALSE``. If ``IN``
returns to ``TRUE`` before ``ET`` reaches ``PT``, ``ET`` is reset and ``Q``
stays ``TRUE``.

Example
-------

.. playground::

   PROGRAM main
      VAR
         myTimer : TOF;
         run : BOOL := TRUE;
         active : BOOL;
         elapsed : TIME;
      END_VAR

      myTimer(IN := run, PT := T#5s, Q => active, ET => elapsed);
   END_PROGRAM

See Also
--------

- :doc:`ton` — on-delay timer
- :doc:`tp` — pulse timer
