===========
Array Types
===========

An array is a fixed-size indexed collection of elements of the same type.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.3.3.1
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   ARRAY [ lower_bound .. upper_bound ] OF element_type

Arrays can be declared inline in variable declarations or as named types.

Example
-------

.. playground::

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

Constant Bounds (Language Extension)
------------------------------------

.. include:: ../../../../includes/requires-dialect-extension.rst

Many PLC vendors allow global constants in place of literal values for
array bounds. IronPLC supports this with the ``--allow-constant-type-params``
flag. The constant must be declared in a
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

Arrays of Arrays
----------------

The element type of an array can be a named array type. Each bracket
selects from one array, so an element of the inner array takes one bracket
per array:

.. playground::

   TYPE
       Row : ARRAY [1..3] OF DINT;
   END_TYPE

   PROGRAM main
       VAR
           rows : ARRAY [1..2] OF Row;
           total : DINT;
       END_VAR

       rows[2][3] := 7;
       total := rows[2][3] + rows[1][1];
   END_PROGRAM

``rows[2]`` is a whole ``Row``. Reading, writing or passing a whole inner
array is not implemented yet and raises
:doc:`P9004 </reference/compiler/problems/P9004>`; access its elements
instead.

See Also
--------

- :doc:`structure-types` — record with named fields
