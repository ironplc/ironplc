====
ACOS
====

Returns the arc cosine (inverse cosine) of a numeric input.

Signature
---------

.. code-block:: text

           ┌─────────┐
       IN ─┤  ACOS   ├─ OUT
           └─────────┘

.. code-block:: text

   FUNCTION ACOS : ANY_REAL
     VAR_INPUT
       IN : ANY_REAL;
     END_VAR
   END_FUNCTION

The return type matches the input type. ``ACOS`` accepts ``REAL``,
``LREAL``.

Description
-----------

Returns the arc cosine of *IN* in radians. The input must be in the
range [-1.0, 1.0]. The result is in the range [0, pi].

Example
-------

.. playground-with-program::
   :vars: result : REAL; value : LREAL;

   result := ACOS(REAL#1.0);   (* result = 0.0 *)
   value := ACOS(LREAL#0.0);   (* value ~ 1.5707963 *)

See Also
--------

* :doc:`cos` — cosine
* :doc:`asin` — arc sine
* :doc:`atan` — arc tangent

References
----------

* IEC 61131-3 §2.5.1.5.2
* `CODESYS: ACOS <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_acos.html>`_
* `Beckhoff TwinCAT 3: ACOS <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2529149579.html>`_
* `Fernhill SCADA: Mathematical Functions <https://www.fernhillsoftware.com/help/iec-61131/common-elements/functions-mathematical.html>`_
