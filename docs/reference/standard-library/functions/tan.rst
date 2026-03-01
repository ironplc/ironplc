===
TAN
===

Returns the tangent of an angle in radians.

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

Returns the tangent of *IN*, where *IN* is an angle expressed in radians.
The result is undefined when *IN* is an odd multiple of pi/2.

Example
-------

.. code-block:: iec61131

   result := TAN(REAL#0.0);          (* result = 0.0 *)
   value := TAN(LREAL#0.7853982);   (* value ~ 1.0 *)

See Also
--------

- :doc:`sin` — sine
- :doc:`cos` — cosine
- :doc:`atan` — arc tangent
