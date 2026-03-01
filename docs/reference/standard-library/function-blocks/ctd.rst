===
CTD
===

Count-down counter. Decrements the counter value ``CV`` on each rising edge of
``CD``. Output ``Q`` becomes ``TRUE`` when ``CV`` reaches or falls below zero.

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
   * - ``CD``
     - ``BOOL``
     - Count-down input (decrements on rising edge)
   * - ``LD``
     - ``BOOL``
     - Load input (loads PV into CV)
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
     - Counter output (TRUE when CV <= 0)
   * - ``CV``
     - ``INT``
     - Current counter value

Behavior
--------

On each rising edge of ``CD``, the counter value ``CV`` is decremented by one.
When ``LD`` is ``TRUE``, the preset value ``PV`` is loaded into ``CV``. The
output ``Q`` is ``TRUE`` when ``CV`` is less than or equal to zero.

Example
-------

.. code-block:: iec61131

   VAR
     counter1 : CTD;
     count_pulse : BOOL;
     load : BOOL;
     done : BOOL;
   END_VAR

   counter1(CD := count_pulse, LD := load, PV := 10);
   done := counter1.Q;

See Also
--------

- :doc:`ctu` — count-up counter
- :doc:`ctud` — count up/down counter
