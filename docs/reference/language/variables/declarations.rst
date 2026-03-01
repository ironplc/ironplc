====================
Variable Declarations
====================

Variables are declared in ``VAR`` / ``END_VAR`` blocks at the beginning of
a program organization unit.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.4.3
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   VAR
       variable_name : type_name ;
       variable_name : type_name := initial_value ;
       var1, var2, var3 : type_name ;
   END_VAR

Multiple variables of the same type can be declared on a single line
separated by commas.

Example
-------

.. code-block:: iec61131

   PROGRAM main
       VAR
           counter : INT := 0;
           limit : INT := 100;
           running : BOOL := TRUE;
           x, y, z : DINT;
       END_VAR

       counter := counter + 1;
   END_PROGRAM

See Also
--------

- :doc:`initial-values` — initialization syntax
- :doc:`scope` — variable scope keywords
