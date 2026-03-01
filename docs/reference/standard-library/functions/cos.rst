===
COS
===

Returns the cosine of an angle in radians.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.5.1.5.2
   * - **Support**
     - Not yet supported

Signatures
----------

.. list-table::
   :header-rows: 1
   :widths: 10 30 30 30

   * - #
     - Input (IN)
     - Return Type
     - Support
   * - 1
     - ``REAL``
     - ``REAL``
     - Not yet supported
   * - 2
     - ``LREAL``
     - ``LREAL``
     - Not yet supported

Description
-----------

Returns the cosine of *IN*, where *IN* is an angle expressed in radians.
The result is in the range [-1.0, 1.0].

Example
-------

.. code-block::

   result := COS(REAL#0.0);          (* result = 1.0 *)
   value := COS(LREAL#3.1415927);   (* value ~ -1.0 *)

See Also
--------

- :doc:`sin` — sine
- :doc:`tan` — tangent
- :doc:`acos` — arc cosine
