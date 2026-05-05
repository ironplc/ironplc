====
CTUD
====

Count up/down counter. Increments ``CV`` on rising edges of ``CU`` and
decrements ``CV`` on rising edges of ``CD``. Provides both an upper-limit
output ``QU`` and a lower-limit output ``QD``.

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
   * - ``CU``
     - ``BOOL``
     - Count-up input. Increments CV on each rising edge.
   * - ``CD``
     - ``BOOL``
     - Count-down input. Decrements CV on each rising edge.
   * - ``R``
     - ``BOOL``
     - Reset input. Resets CV to zero while TRUE.
   * - ``LD``
     - ``BOOL``
     - Load input. Loads PV into CV while TRUE.
   * - ``PV``
     - ``INT``
     - Preset value. The upper threshold for QU and the value loaded by LD.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``QU``
     - ``BOOL``
     - TRUE when the current counter value CV is greater than or equal to PV.
   * - ``QD``
     - ``BOOL``
     - TRUE when the current counter value CV is less than or equal to zero.
   * - ``CV``
     - ``INT``
     - Current counter value.

Behavior
--------

On each rising edge of ``CU``, the counter value ``CV`` is incremented by one.
On each rising edge of ``CD``, ``CV`` is decremented by one. When ``R`` is
``TRUE``, ``CV`` is reset to zero. When ``LD`` is ``TRUE``, the preset value
``PV`` is loaded into ``CV``. The output ``QU`` is ``TRUE`` when ``CV`` is
greater than or equal to ``PV``. The output ``QD`` is ``TRUE`` when ``CV`` is
less than or equal to zero.

Typed variants ``CTUD_DINT``, ``CTUD_LINT``, ``CTUD_UDINT``, and ``CTUD_ULINT``
use the corresponding integer type for ``PV`` and ``CV``.

Example
-------

This example counts up with ``CU`` held ``TRUE``. After the first scan,
``CV`` is 1 which reaches ``PV``, so ``at_max`` is ``TRUE``.

.. playground::

   PROGRAM main
      VAR
         counter : CTUD;
         at_max : BOOL;
         at_min : BOOL;
         count : INT;
      END_VAR

      counter(CU := TRUE, CD := FALSE, R := FALSE, LD := FALSE, PV := 1,
              QU => at_max, QD => at_min, CV => count);
      (* After first scan: count = 1, at_max = TRUE, at_min = FALSE *)
   END_PROGRAM

See Also
--------

- :doc:`ctu` — count-up counter
- :doc:`ctd` — count-down counter
