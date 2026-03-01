==============
Initial Values
==============

Variables can be assigned initial values in their declaration using the
``:=`` operator.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.4.3
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   variable_name : type_name := initial_value ;

The initial value must be a constant expression compatible with the
variable's type.

Example
-------

.. code-block:: iec61131

   PROGRAM main
       VAR
           counter : INT := 0;
           name : STRING := 'default';
           active : BOOL := TRUE;
           delay : TIME := T#100ms;
       END_VAR

       counter := counter + 1;
   END_PROGRAM

If no initial value is specified, the variable is initialized to the
default value of its type (typically zero or empty).

See Also
--------

- :doc:`declarations` — variable declaration syntax
- :doc:`/reference/language/data-types/index` — default values for each type
