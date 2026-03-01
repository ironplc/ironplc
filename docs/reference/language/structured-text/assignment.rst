==========
Assignment
==========

The assignment statement stores the value of an expression in a variable.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.1
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   variable := expression ;

The left-hand side must be a variable that is writable (not declared as
``CONSTANT``). The right-hand side is any expression whose type is compatible
with the variable.

Description
-----------

The ``:=`` operator evaluates the expression on the right and assigns the
result to the variable on the left. The statement is terminated with a
semicolon.

Example
-------

.. code-block:: iec61131

   PROGRAM main
       VAR
           counter : INT := 0;
           limit : INT := 100;
           active : BOOL;
       END_VAR

       counter := counter + 1;
       active := counter < limit;
   END_PROGRAM

See Also
--------

- :doc:`/reference/language/variables/declarations` — variable declaration syntax
- :doc:`arithmetic-operators` — arithmetic expressions
- :doc:`comparison-operators` — comparison expressions
