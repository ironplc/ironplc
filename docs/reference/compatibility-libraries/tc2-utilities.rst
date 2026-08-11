=============
Tc2_Utilities
=============

Compatibility library for Beckhoff's TwinCAT 3 ``Tc2_Utilities`` PLC
library: utility and formatting functions. Activated by a project
reference or ``--library Tc2_Utilities``.

Functions
---------

.. list-table::
   :header-rows: 1
   :widths: 25 30 45

   * - Name
     - Signature
     - Description
   * - ``LREAL_TO_FMTSTR``
     - ``LREAL_TO_FMTSTR(in : LREAL, iPrecision : INT, bRound : BOOL) : STRING``
     - Formats *in* as a fixed-point decimal string with *iPrecision*
       digits after the decimal point (clamped to 0..15). When *bRound*
       is ``TRUE`` the last place is rounded (half away from zero);
       when ``FALSE`` it is truncated toward zero. Values that cannot be
       rendered (NaN, infinities, magnitudes of 2\ :sup:`63` or more)
       return an empty string.

The formal parameter names (``in``, ``iPrecision``, ``bRound``) match the
vendor's documented names, so source that passes arguments by name
resolves unchanged.

Example
-------

.. code-block::

   text := LREAL_TO_FMTSTR(in := 3.14159, iPrecision := 2, bRound := TRUE);
   (* text = '3.14' *)

See Also
--------

* :doc:`index` — how compatibility libraries are activated
* `Beckhoff Information System <https://infosys.beckhoff.com/>`_ — the
  vendor documentation this interface reproduces (TwinCAT 3 → PLC
  libraries → Tc2_Utilities)
