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

   FUNCTION LIMIT : ANY
     VAR_INPUT
       MN : ANY;
       IN : ANY;
       MX : ANY;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``LIMIT`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``USINT``, ``UINT``, ``UDINT``, ``ULINT``,
``REAL``, ``LREAL``. All three inputs must share the same type.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``MN``
     - ``ANY_MAGNITUDE``
     - Lower bound of the range.
   * - ``IN``
     - ``ANY_MAGNITUDE``
     - Value to clamp.
   * - ``MX``
     - ``ANY_MAGNITUDE``
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
     - ``ANY_MAGNITUDE``
     - IN clamped to [MN, MX]. Same type as the inputs.

Description
-----------

``LIMIT(MN, IN, MX)`` clamps *IN* to the range [*MN*, *MX*]. The
function returns:

- *MN* if *IN* < *MN*
- *MX* if *IN* > *MX*
- *IN* otherwise

The behavior is undefined if *MN* > *MX*.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := LIMIT(0, 50, 100);    (* result = 50 *)
   result := LIMIT(0, -10, 100);   (* result = 0 *)
   result := LIMIT(0, 200, 100);   (* result = 100 *)

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
