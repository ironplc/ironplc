===
SEL
===

Binary selection — selects one of two inputs based on a Boolean selector.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.5
   * - **Support**
     - Not yet supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 15 15 15 15 30

   * - #
     - Input (G)
     - Input (IN0)
     - Input (IN1)
     - Return Type
     - Support
   * - 1
     - ``BOOL``
     - *ANY*
     - *ANY*
     - *ANY*
     - Not yet supported

Description
-----------

``SEL(G, IN0, IN1)`` returns *IN0* if *G* is ``FALSE``, or *IN1* if
*G* is ``TRUE``. The types of *IN0* and *IN1* must be the same, and
the return type matches the input type.

This function is polymorphic: it works with any data type for the
selected inputs.

Example
-------

.. code-block:: iec61131

   result := SEL(TRUE, 10, 20);     (* result = 20 *)
   result := SEL(FALSE, 10, 20);    (* result = 10 *)
   flag := SEL(cond, FALSE, TRUE);  (* conditional Boolean *)

See Also
--------

- :doc:`mux` — multiplexer (selects from multiple inputs)
- :doc:`max` — maximum of two values
- :doc:`min` — minimum of two values
