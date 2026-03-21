===========
Array Types
===========

An array is a fixed-size indexed collection of elements of the same type.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.3.3.1
   * - **Support**
     - Not yet supported

Syntax
------

.. code-block:: bnf

   ARRAY [ lower_bound .. upper_bound ] OF element_type

Arrays can be declared inline in variable declarations or as named types.

Example
-------

.. code-block::

   TYPE
       TenInts : ARRAY [1..10] OF INT;
   END_TYPE

   PROGRAM main
       VAR
           values : ARRAY [0..9] OF DINT;
           matrix : ARRAY [1..3, 1..3] OF REAL;
       END_VAR

       values[0] := 42;
       values[5] := values[0] + 1;
   END_PROGRAM

Multi-dimensional arrays use comma-separated ranges in the index
specification.

Constant Bounds (Vendor Extension)
----------------------------------

.. include:: ../../../includes/requires-vendor-extension.rst

Many PLC vendors allow global constants in place of literal values for
array bounds. IronPLC supports this with the ``--allow-constant-type-params``
flag (or ``--allow-all``). The constant must be declared in a
``VAR_GLOBAL CONSTANT`` block.

.. playground::

   VAR_GLOBAL CONSTANT
     ARRAY_SIZE : INT := 10;
   END_VAR

   FUNCTION_BLOCK fb1
     VAR_EXTERNAL CONSTANT
       ARRAY_SIZE : INT;
     END_VAR
     VAR
       data : ARRAY[1..ARRAY_SIZE] OF INT;
     END_VAR
   END_FUNCTION_BLOCK

   PROGRAM main
     VAR
       instance : fb1;
     END_VAR
   END_PROGRAM

See Also
--------

- :doc:`structure-types` — record with named fields
