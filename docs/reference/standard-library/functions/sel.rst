===
SEL
===

Binary selection — selects one of two inputs based on a Boolean selector.

Signature
---------

.. code-block:: text

            ┌─────────┐
         G ─┤         │
       IN0 ─┤   SEL   ├─ OUT
       IN1 ─┤         │
            └─────────┘

.. code-block:: text

   FUNCTION SEL : ANY
     VAR_INPUT
       G   : BOOL;
       IN0 : ANY;
       IN1 : ANY;
     END_VAR
   END_FUNCTION

The return type matches the type of *IN0* and *IN1*, which must be the
same. ``SEL`` is polymorphic over any data type.

Description
-----------

``SEL(G, IN0, IN1)`` returns *IN0* if *G* is ``FALSE``, or *IN1* if
*G* is ``TRUE``. The types of *IN0* and *IN1* must be the same, and
the return type matches the input type.

This function is polymorphic: it works with any data type for the
selected inputs.

Example
-------

.. playground-with-program::
   :vars: result : DINT;

   result := SEL(TRUE, 10, 20);     (* result = 20 *)
   result := SEL(FALSE, 10, 20);    (* result = 10 *)

See Also
--------

* :doc:`mux` — multiplexer (selects from multiple inputs)
* :doc:`max` — maximum of two values
* :doc:`min` — minimum of two values

References
----------

* IEC 61131-3 §2.5.1.5.5
* `CODESYS: SEL <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_operator_sel.html>`_
* `Beckhoff TwinCAT 3: SEL <https://infosys.beckhoff.com/content/1033/tc3_plc_intro/2528956043.html>`_
* `Fernhill SCADA: SEL <https://www.fernhillsoftware.com/help/iec-61131/common-elements/selection-functions/select.html>`_
