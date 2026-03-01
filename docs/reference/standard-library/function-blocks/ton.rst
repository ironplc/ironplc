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
     - Timer output (TRUE when elapsed time >= PT)
   * - ``ET``
     - ``TIME``
     - Elapsed time

Behavior
--------

When ``IN`` transitions to ``TRUE``, the elapsed time ``ET`` begins counting
from ``T#0s``. When ``ET`` reaches the preset time ``PT``, the output ``Q``
becomes ``TRUE``. If ``IN`` returns to ``FALSE`` before ``ET`` reaches ``PT``,
both ``ET`` and ``Q`` are reset.

Example
-------

.. code-block:: iec61131

   VAR
     timer1 : TON;
     start : BOOL;
     done : BOOL;
   END_VAR

   timer1(IN := start, PT := T#5s);
   done := timer1.Q;

See Also
--------

- :doc:`tof` — off-delay timer
- :doc:`tp` — pulse timer
