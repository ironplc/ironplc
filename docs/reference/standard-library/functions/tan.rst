===
TAN
===

Returns the tangent of an angle in radians.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤   TAN   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION TAN : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``TAN`` accepts ``REAL``,
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
     - The tangent of IN. Same type as IN.

Description
-----------

Returns the tangent of *IN*, where *IN* is an angle expressed in radians.
The result is undefined when *IN* is an odd multiple of pi/2.

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := TAN(REAL#0.0);          (* result = 0.0 *)
   value := TAN(LREAL#0.7853982);   (* value ~ 1.0 *)

See Also
--------

* :doc:`sin` — sine
* :doc:`cos` — cosine
* :doc:`atan` — arc tangent

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: TAN <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_tan.html>`_
* `Beckhoff TwinCAT 3: TAN <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529138827.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
