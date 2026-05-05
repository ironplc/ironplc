===
SIN
===

Returns the sine of an angle in radians.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤   SIN   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION SIN : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``SIN`` accepts ``REAL``,
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
     - The sine of IN, in the range [-1.0, 1.0]. Same type as IN.

Description
-----------

Returns the sine of *IN*, where *IN* is an angle expressed in radians.
The result is in the range [-1.0, 1.0].

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := SIN(REAL#0.0);          (* result = 0.0 *)
   value := SIN(LREAL#1.5707963);   (* value ~ 1.0 *)

See Also
--------

* :doc:`cos` — cosine
* :doc:`tan` — tangent
* :doc:`asin` — arc sine

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: SIN <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sin.html>`_
* `Beckhoff TwinCAT 3: SIN <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/18014401038610059.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
