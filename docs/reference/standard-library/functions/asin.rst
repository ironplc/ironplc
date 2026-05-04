====
ASIN
====

Returns the arc sine (inverse sine) of a numeric input.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤  ASIN   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION ASIN : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``ASIN`` accepts ``REAL``,
``LREAL``.

Description
-----------

Returns the arc sine of *IN* in radians. The input must be in the
range [-1.0, 1.0]. The result is in the range [-pi/2, pi/2].

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := ASIN(REAL#0.0);   (* result = 0.0 *)
   value := ASIN(LREAL#1.0);   (* value ~ 1.5707963 *)

See Also
--------

* :doc:`sin` — sine
* :doc:`acos` — arc cosine
* :doc:`atan` — arc tangent

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: ASIN <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_asin.html>`_
* `Beckhoff TwinCAT 3: ASIN <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529144203.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
