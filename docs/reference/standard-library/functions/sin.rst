===
SIN
===

Returns the sine of an angle in radians.

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

Returns the sine of *IN*, where *IN* is an angle expressed in radians.
The result is in the range [-1.0, 1.0].

Example
-------

.. code-block::

   result := SIN(REAL#0.0);          (* result = 0.0 *)
   value := SIN(LREAL#1.5707963);   (* value ~ 1.0 *)

See Also
--------

- :doc:`cos` — cosine
- :doc:`tan` — tangent
- :doc:`asin` — arc sine
