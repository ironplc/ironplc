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

Each subscript is an integer. A ``BOOL`` subscript is error
:doc:`P4086 </reference/compiler/problems/P4086>`, and a real literal
subscript such as ``values[1.5]`` is error
:doc:`P4094 </reference/compiler/problems/P4094>`.

Only an array can be subscripted. A subscript on anything else, such as a
``DINT``, a ``STRING`` or a structure, is error
:doc:`P4097 </reference/compiler/problems/P4097>`.

Arrays of Arrays
----------------

The element type of an array can be another array type. Each subscript list
selects from one array. An element of an array of arrays takes one list for
each array, and an element of a multi-dimensional array takes one list with a
subscript for each dimension:

.. code-block::

   TYPE
       Row : ARRAY [1..3] OF DINT;
   END_TYPE

   PROGRAM main
       VAR
           rows : ARRAY [1..2] OF Row;
           matrix : ARRAY [1..2, 1..3] OF DINT;
           row : Row;
       END_VAR

       rows[2][3] := 7;    (* an element of the inner array *)
       row := rows[2];     (* a whole inner array, a Row *)
       matrix[2, 3] := 7;  (* one list for both dimensions *)
   END_PROGRAM

``rows[2, 3]`` and ``matrix[2][3]`` give the wrong number of subscripts for
their arrays, which is error :doc:`P4085 </reference/compiler/problems/P4085>`.

Initial Values
--------------

An array takes its initial values as a list in brackets. The list gives the
element values in order, and for a multi-dimensional array it gives them row
by row. A count before a value in parentheses repeats the value: ``3(0)``
gives three zeros. The list may give fewer values than the array has
elements. Each remaining element starts at its type's default value.

The list follows the array's type, whether the type is an ``ARRAY`` written in
place or the name of an array type. The declaration of an array type and a
structure field of an array type take a list the same way.

.. playground::

   TYPE
       Triple : ARRAY [1..3] OF DINT;
   END_TYPE

   PROGRAM main
       VAR
           limits : ARRAY [1..3] OF DINT := [10, 20, 30];
           offsets : Triple := [2(0), 5];
           grid : ARRAY [1..2, 1..2] OF INT := [11, 12, 21, 22];
           total : DINT;
       END_VAR

       total := limits[2] + offsets[3] + grid[2, 1];  (* 20 + 5 + 21 *)
   END_PROGRAM

More values than the array has elements is error
:doc:`P4077 </reference/compiler/problems/P4077>`. A list after the name of a
type that is not an array, such as a structure, is error
:doc:`P4022 </reference/compiler/problems/P4022>`.

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

Radix Bounds (Language Extension)
---------------------------------

.. include:: ../../../../includes/requires-dialect-extension.rst

With the ``--allow-radix-subrange-bounds`` flag, a bound can be written in
hex, binary, or octal. Without the flag, such a bound is reported as
:doc:`P4096 </reference/compiler/problems/P4096>`.

.. code-block::

   PROGRAM main
     VAR
       nibbles : ARRAY[16#0..16#F] OF BYTE;
     END_VAR
   END_PROGRAM

See Also
--------

- :doc:`structure-types` — record with named fields
