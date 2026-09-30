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

Subscripts
----------

A subscript selects one element, and an array takes one subscript per
dimension: ``values[5]``, ``matrix[1, 2]``. IronPLC also accepts the
subscripts of a multi-dimensional array one bracket at a time, so
``matrix[1][2]`` selects the same element as ``matrix[1, 2]``.

A subscript on a variable or field that is not an array raises
:doc:`P4070 </reference/compiler/problems/P4070>`, and a number of
subscripts that does not match the array's dimensions raises
:doc:`P4071 </reference/compiler/problems/P4071>`.

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

See Also
--------

- :doc:`structure-types` — record with named fields
- :doc:`/reference/compiler/problems/P4070` — subscript on a variable that is not an array
- :doc:`/reference/compiler/problems/P4071` — wrong number of subscripts
