=====
Coils
=====

Coils are output elements in ladder diagrams that write the state of a
boolean variable based on the power flow reaching them.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.2
   * - **Support**
     - Not yet supported

Coil Types
----------

.. list-table::
   :header-rows: 1
   :widths: 25 25 50

   * - Type
     - Symbol
     - Description
   * - Output
     - ``--( )--``
     - Sets variable to the power flow state
   * - Negated
     - ``--(/)--``
     - Sets variable to the inverse of power flow
   * - Set (latch)
     - ``--(S)--``
     - Sets variable to TRUE; remains TRUE until reset
   * - Reset (unlatch)
     - ``--(R)--``
     - Resets variable to FALSE

Description
-----------

An output coil writes TRUE when power reaches it and FALSE when it does
not. Set and reset coils provide latching behavior equivalent to the
:doc:`/reference/standard-library/function-blocks/sr` and
:doc:`/reference/standard-library/function-blocks/rs` function blocks.

See Also
--------

- :doc:`contacts` — input elements
- :doc:`rungs` — logic lines
