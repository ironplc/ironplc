===
INT
===

16-bit signed integer.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 16 bits
   * - **Range**
     - -32,768 to 32,767
   * - **Default**
     - 0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   INT#42
   INT#-1000
   INT#16#FF

Example
-------

.. playground-with-program::
   :vars: temperature : INT; offset : INT; adjusted : INT;

   temperature := INT#1500;
   offset := INT#-50;
   adjusted := temperature + offset;  (* adjusted = 1450 *)

See Also
--------

- :doc:`sint` — 8-bit signed integer
- :doc:`dint` — 32-bit signed integer
- :doc:`uint` — 16-bit unsigned integer
