===
COS
===

Returns the cosine of an angle in radians.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤   COS   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION COS : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``COS`` accepts ``REAL``,
``LREAL``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``ANY_REAL``
     - Angle in radians.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_REAL``
     - The cosine of IN, in the range [-1.0, 1.0]. Same type as IN.

Description
-----------

Returns the cosine of *IN*, where *IN* is an angle expressed in radians.
The result is in the range [-1.0, 1.0].

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := COS(REAL#0.0);          (* result = 1.0 *)
   value := COS(LREAL#3.1415927);   (* value ~ -1.0 *)

See Also
--------

* :doc:`sin` — sine
* :doc:`tan` — tangent
* :doc:`acos` — arc cosine

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: COS <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_cos.html>`_
* `Beckhoff TwinCAT 3: COS <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/18014401038615435.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
