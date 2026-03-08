====
UINT
====

16-bit unsigned integer.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 16 bits
   * - **Range**
     - 0 to 65,535
   * - **Default**
     - 0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   UINT#42
   UINT#65535
   UINT#16#FFFF

Example
-------

.. playground-with-program::
   :vars: position : UINT; step_size : UINT; new_pos : UINT;

   position := UINT#1000;
   step_size := UINT#250;
   new_pos := position + step_size;  (* new_pos = 1250 *)

See Also
--------

- :doc:`usint` — 8-bit unsigned integer
- :doc:`udint` — 32-bit unsigned integer
- :doc:`int` — 16-bit signed integer
