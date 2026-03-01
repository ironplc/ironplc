===
CTU
===

Count-up counter. Increments the counter value ``CV`` on each rising edge of
``CU``. Output ``Q`` becomes ``TRUE`` when ``CV`` reaches or exceeds the
preset value ``PV``.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.2.3.4
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

Example
-------

.. code-block:: iec61131

   VAR
     counter1 : CTU;
     count_pulse : BOOL;
     reset : BOOL;
     done : BOOL;
   END_VAR

   counter1(CU := count_pulse, R := reset, PV := 10);
   done := counter1.Q;

See Also
--------

- :doc:`ctd` — count-down counter
- :doc:`ctud` — count up/down counter
