===
ABS
===

Returns the absolute value of a numeric input.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤   ABS   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION ABS : ANY_NUM
     VAR_INPUT
       IN : ANY_NUM;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``ABS`` accepts ``SINT``,
``INT``, ``DINT``, ``LINT``, ``REAL``, ``LREAL``.

.. rubric:: Inputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - ``IN``
     - ``ANY_NUM``
     - The numeric value to compute the absolute value of.

.. rubric:: Outputs

.. list-table::
   :header-rows: 1
   :widths: 20 20 60
   :align: left

   * - Name
     - Type
     - Description
   * - Return value
     - ``ANY_NUM``
     - The non-negative magnitude of IN. Same type as IN.

Description
-----------

Returns the absolute value of *IN*. For signed integer types, the result
of ``ABS`` applied to the most negative value is undefined because
the positive value cannot be represented.

Example
-------

.. playground-with-program::
   :vars: result : DINT; value : REAL;

   result := ABS(-42);    (* result = 42 *)
   value := ABS(REAL#-3.14);  (* value = 3.14 *)

See Also
--------

* :doc:`sqrt` — square root
* :doc:`expt` — exponentiation

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: ABS <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_abs.html>`_
* `Beckhoff TwinCAT 3: ABS <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529095819.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
