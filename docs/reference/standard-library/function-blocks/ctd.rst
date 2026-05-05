===
CTD
===

Count-down counter. Decrements the counter value ``CV`` on each rising edge of
``CD``. Output ``Q`` becomes ``TRUE`` when ``CV`` reaches or falls below zero.

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
   * - ``CD``
     - ``BOOL``
     - Count-down input. Decrements CV on each rising edge.
   * - ``LD``
     - ``BOOL``
     - Load input. Loads PV into CV while TRUE.
   * - ``PV``
     - ``INT``
     - Preset value. The starting value loaded into CV when LD is TRUE.

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
     - TRUE when the current counter value CV is less than or equal to zero.
   * - ``CV``
     - ``INT``
     - Current counter value.

Behavior
--------

On each rising edge of ``CD``, the counter value ``CV`` is decremented by one.
When ``LD`` is ``TRUE``, the preset value ``PV`` is loaded into ``CV``. The
output ``Q`` is ``TRUE`` when ``CV`` is less than or equal to zero.

Typed variants ``CTD_DINT``, ``CTD_LINT``, ``CTD_UDINT``, and ``CTD_ULINT``
use the corresponding integer type for ``PV`` and ``CV``.

Example
-------

This example loads the preset value into the counter. After loading, ``CV`` is
3 which is above zero, so ``expired`` is ``FALSE``.

.. playground::

   PROGRAM main
      VAR
         counter : CTD;
         expired : BOOL;
         count : INT;
      END_VAR

      counter(CD := FALSE, LD := TRUE, PV := 3, Q => expired, CV => count);
      (* After first scan: count = 3, expired = FALSE *)
   END_PROGRAM

See Also
--------

- :doc:`ctu` — count-up counter
- :doc:`ctud` — count up/down counter
