====
ACOS
====

Returns the arc cosine (inverse cosine) of a numeric input.

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

Returns the arc cosine of *IN* in radians. The input must be in the
range [-1.0, 1.0]. The result is in the range [0, pi].

Example
-------

.. code-block::

   result := ACOS(REAL#1.0);   (* result = 0.0 *)
   value := ACOS(LREAL#0.0);   (* value ~ 1.5707963 *)

See Also
--------

- :doc:`cos` — cosine
- :doc:`asin` — arc sine
- :doc:`atan` — arc tangent
