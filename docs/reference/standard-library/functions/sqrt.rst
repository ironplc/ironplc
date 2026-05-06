====
SQRT
====

Returns the square root of a numeric input.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤  SQRT   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION SQRT : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``SQRT`` accepts ``REAL``,
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
     - The non-negative value to compute the square root of.

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
     - The square root of IN. Same type as IN.

Description
-----------

Returns the square root of *IN*. The input must be non-negative;
the result of ``SQRT`` applied to a negative value is undefined.

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := SQRT(REAL#9.0);    (* result = 3.0 *)
   value := SQRT(LREAL#2.0);   (* value = 1.41421356... *)

See Also
--------

* :doc:`abs` — absolute value
* :doc:`expt` — exponentiation
* :doc:`exp` — natural exponential

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: SQRT <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sqrt.html>`_
* `Beckhoff TwinCAT 3: SQRT <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529101195.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
