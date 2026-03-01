====
LINT
====

64-bit signed integer.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 64 bits
   * - **Range**
     - -9,223,372,036,854,775,808 to 9,223,372,036,854,775,807
   * - **Default**
     - 0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block:: iec61131

   LINT#42
   LINT#-100000000
   LINT#16#FFFFFFFF

See Also
--------

- :doc:`dint` — 32-bit signed integer
- :doc:`ulint` — 64-bit unsigned integer
