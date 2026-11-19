=====
LIMIT
=====

Clamps a value to a specified range.

Signature
---------

.. code-block:: text

            ┌─────────┐
        MN ─┤         │
        IN ─┤  LIMIT  ├─ OUT
        MX ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION LIMIT : ANY_ELEMENTARY
     VAR_INPUT
       MN : ANY_ELEMENTARY;
       IN : ANY_ELEMENTARY;
       MX : ANY_ELEMENTARY;
     END_VAR
   END_FUNCTION

.. include:: ../../../includes/ordered-selection-input-types.rst

.. include:: ../../../includes/widest-input-return-type.rst

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``MN``
     - ``ANY_ELEMENTARY``
     - Lower bound of the range.
   * - ``IN``
     - ``ANY_ELEMENTARY``
     - Value to clamp.
   * - ``MX``
     - ``ANY_ELEMENTARY``
     - Upper bound of the range.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_ELEMENTARY``
     - IN clamped to [MN, MX].

Description
-----------

``LIMIT(MN, IN, MX)`` clamps *IN* to the range [*MN*, *MX*]. The
function returns:

- *MN* if *IN* < *MN*
- *MX* if *IN* > *MX*
- *IN* otherwise

``LIMIT`` is defined as ``MIN(MAX(IN, MN), MX)``, so when *MN* > *MX* the
result is *MX*.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := LIMIT(0, 50, 100);    (* result = 50 *)
   result := LIMIT(0, -10, 100);   (* result = 0 *)
   result := LIMIT(0, 200, 100);   (* result = 100 *)

A period kept within its range:

.. playground-with-program::
   :vars: period : TIME;

   period := LIMIT(T#100ms, T#10s, T#5s);    (* period = T#5s *)

See Also
--------

* :doc:`max` — maximum of two values
* :doc:`min` — minimum of two values
* :doc:`sel` — binary selection

References
----------

* IEC 61131-3 §2.5.1.5.5
* `CODESYS: LIMIT <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_limit.html>`_
* `Beckhoff TwinCAT 3: LIMIT <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528972171.html>`_
* `Fernhill SCADA: LIMIT <https://www.fernhillsoftware.com/help/iec-61131/common-elements/selection-functions/limit.html>`_
