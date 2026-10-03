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

.. playground::

   PROGRAM main
       VAR
           counter : INT := 0;
           active : BOOL := TRUE;
           scale : REAL := 1.5;
       END_VAR

       counter := counter + 1;
   END_PROGRAM

If no initial value is specified, the variable is initialized to the
default value of its type (typically zero or empty).

Named Constants (Language Extension)
------------------------------------

.. include:: ../../../includes/requires-dialect-extension.rst

The standard allows only a literal as an initial value. With
``--allow-constant-initializer-expressions``, an initial value can also
be the name of a constant, or an expression over constants and literals.
The compiler replaces it with the value it denotes.

.. playground::
   :allows: constant-initializer-expressions

   PROGRAM main
       VAR CONSTANT
           L : DINT := -3;
       END_VAR
       VAR
           x : DINT := L;
       END_VAR
   END_PROGRAM

The name must be that of a variable declared ``CONSTANT``
(:doc:`P4038 </reference/compiler/problems/P4038>` otherwise), and the
constant's type must be one that can be assigned to the variable
(:doc:`P4022 </reference/compiler/problems/P4022>` otherwise): an ``INT``
constant can initialize a ``LINT`` but a ``UDINT`` constant cannot
initialize an ``INT``. For a variable of an enumerated type, a bare name
is one of the type's values.

See Also
--------

- :doc:`declarations` — variable declaration syntax
- :doc:`/reference/language/data-types/index` — default values for each type
