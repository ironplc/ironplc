===============
Structure Types
===============

A structure type defines a record with named fields of potentially
different types.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.3.3.1
   * - **Support**
     - Partial

Syntax
------

.. code-block:: bnf

   TYPE
       type_name : STRUCT
           field_name : field_type ;
           ...
       END_STRUCT ;
   END_TYPE

Example
-------

.. code-block:: iec61131

   TYPE
       Point : STRUCT
           X : REAL;
           Y : REAL;
       END_STRUCT;
   END_TYPE

   PROGRAM main
       VAR
           origin : Point;
       END_VAR

       origin.X := 0.0;
       origin.Y := 0.0;
   END_PROGRAM

Fields are accessed using dot notation. Each field name must be unique
within the structure.

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P2001` — Duplicate element name in structure

See Also
--------

- :doc:`array-types` — fixed-size indexed collection
- :doc:`enumerated-types` — named set of values
