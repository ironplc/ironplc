===
CTU
===

Count-up counter. Increments the counter value ``CV`` on each rising edge of
``CU``. Output ``Q`` becomes ``TRUE`` when ``CV`` reaches or exceeds the
preset value ``PV``.

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
   * - ``CU``
     - ``BOOL``
     - Count-up input (increments on rising edge)
   * - ``R``
     - ``BOOL``
     - Reset input (sets CV to 0)
   * - ``PV``
     - ``INT``
     - Preset value

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
     - Counter output (TRUE when CV >= PV)
   * - ``CV``
     - ``INT``
     - Current counter value

Behavior
--------

On each rising edge of ``CU``, the counter value ``CV`` is incremented by one.
When ``R`` is ``TRUE``, ``CV`` is reset to zero. The output ``Q`` is ``TRUE``
when ``CV`` is greater than or equal to the preset value ``PV``.

Typed variants ``CTU_DINT``, ``CTU_LINT``, ``CTU_UDINT``, and ``CTU_ULINT``
use the corresponding integer type for ``PV`` and ``CV``.

Example
-------

This example counts up with ``CU`` held ``TRUE``. After the first scan, ``CV``
is 1 which equals ``PV``, so ``done`` becomes ``TRUE``.

.. playground::

   PROGRAM main
      VAR
         counter : CTU;
         done : BOOL;
         count : INT;
      END_VAR

      counter(CU := TRUE, R := FALSE, PV := 1, Q => done, CV => count);
      (* After first scan: count = 1, done = TRUE *)
   END_PROGRAM

See Also
--------

- :doc:`ctd` — count-down counter
- :doc:`ctud` — count up/down counter
