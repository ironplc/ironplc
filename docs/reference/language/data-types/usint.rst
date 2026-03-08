=====
USINT
=====

8-bit unsigned integer.

.. list-table::
   :widths: 30 70

   * - **Size**
     - 8 bits
   * - **Range**
     - 0 to 255
   * - **Default**
     - 0
   * - **IEC 61131-3**
     - Section 2.3.1
   * - **Support**
     - Supported

Literals
--------

.. code-block::

   USINT#42
   USINT#255
   USINT#16#FF

Example
-------

.. playground-with-program::
   :vars: level : USINT; max_level : USINT; clamped : USINT;

   level := USINT#200;
   max_level := USINT#255;
   clamped := level + USINT#30;  (* clamped = 230 *)

See Also
--------

- :doc:`uint` — 16-bit unsigned integer
- :doc:`sint` — 8-bit signed integer
