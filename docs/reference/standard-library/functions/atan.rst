====
ATAN
====

Returns the arc tangent (inverse tangent) of a numeric input.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤  ATAN   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION ATAN : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``ATAN`` accepts ``REAL``,
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
     - The numeric value to compute the arc tangent of.

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
     - The arc tangent of IN in radians, in the range [-pi/2, pi/2]. Same type as IN.

Description
-----------

Returns the arc tangent of *IN* in radians. The result is in the
range [-pi/2, pi/2].

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := ATAN(REAL#0.0);   (* result = 0.0 *)
   value := ATAN(LREAL#1.0);   (* value ~ 0.7853982 *)

See Also
--------

* :doc:`tan` — tangent
* :doc:`asin` — arc sine
* :doc:`acos` — arc cosine

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: ATAN <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_atan.html>`_
* `Beckhoff TwinCAT 3: ATAN <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529154955.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
