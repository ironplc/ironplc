==========
Tc2_System
==========

Compatibility library for Beckhoff's TwinCAT 3 ``Tc2_System`` PLC
library. Activated by a project reference or ``--library Tc2_System``.

Constants
---------

.. list-table::
   :header-rows: 1
   :widths: 20 20 60

   * - Name
     - Type
     - Description
   * - ``PI``
     - ``LREAL``
     - The mathematical constant π (3.14159265358979...), as a global
       constant.

Example
-------

.. code-block::

   FUNCTION F_DegreesToRadians : LREAL
   VAR_INPUT
       degrees : LREAL;
   END_VAR
   F_DegreesToRadians := degrees * PI / 180.0;
   END_FUNCTION

See Also
--------

* :doc:`index` — how compatibility libraries are activated
* `Beckhoff TwinCAT 3: Tc2_System PI constant <https://infosys.beckhoff.com/english.php?content=../content/1033/tcplclib_tc2_system/31084171.html&id=>`_
  — the vendor documentation this interface reproduces
