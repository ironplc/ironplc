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
     - Timer enable input
   * - ``PT``
     - ``TIME``
     - Preset time

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
     - Timer output (TRUE during off-delay period)
   * - ``ET``
     - ``TIME``
     - Elapsed time

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
