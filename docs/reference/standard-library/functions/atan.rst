====
ATAN
====

Returns the arc tangent (inverse tangent) of a numeric input.

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

Returns the arc tangent of *IN* in radians. The result is in the
range [-pi/2, pi/2].

Example
-------

.. code-block:: iec61131

   result := ATAN(REAL#0.0);   (* result = 0.0 *)
   value := ATAN(LREAL#1.0);   (* value ~ 0.7853982 *)

See Also
--------

- :doc:`tan` — tangent
- :doc:`asin` — arc sine
- :doc:`acos` — arc cosine
