====
CTUD
====

Count up/down counter. Increments ``CV`` on rising edges of ``CU`` and
decrements ``CV`` on rising edges of ``CD``. Provides both an upper-limit
output ``QU`` and a lower-limit output ``QD``.

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
   * - ``CD``
     - ``BOOL``
     - Count-down input (decrements on rising edge)
   * - ``R``
     - ``BOOL``
     - Reset input (sets CV to 0)
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
   * - ``QU``
     - ``BOOL``
     - Upper-limit output (TRUE when CV >= PV)
   * - ``QD``
     - ``BOOL``
     - Lower-limit output (TRUE when CV <= 0)
   * - ``CV``
     - ``INT``
     - Current counter value

Behavior
--------

On each rising edge of ``CU``, the counter value ``CV`` is incremented by one.
On each rising edge of ``CD``, ``CV`` is decremented by one. When ``R`` is
``TRUE``, ``CV`` is reset to zero. When ``LD`` is ``TRUE``, the preset value
``PV`` is loaded into ``CV``. The output ``QU`` is ``TRUE`` when ``CV`` is
greater than or equal to ``PV``. The output ``QD`` is ``TRUE`` when ``CV`` is
less than or equal to zero.

Example
-------

.. code-block::

   VAR
     counter1 : CTUD;
     up_pulse : BOOL;
     down_pulse : BOOL;
     reset : BOOL;
     load : BOOL;
     at_max : BOOL;
     at_min : BOOL;
   END_VAR

   counter1(CU := up_pulse, CD := down_pulse, R := reset, LD := load, PV := 100);
   at_max := counter1.QU;
   at_min := counter1.QD;

See Also
--------

- :doc:`ctu` — count-up counter
- :doc:`ctd` — count-down counter
