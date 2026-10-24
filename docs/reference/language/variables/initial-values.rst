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

Named Constants
---------------

The initial value may name a variable declared with the ``CONSTANT``
qualifier, or be an expression over literals and such constants. The
compiler replaces the name with the constant's value when it compiles the
program, so the variable starts at that value. A ``STRING`` or ``WSTRING``
variable and the default of a ``TYPE`` declaration take a constant's name
the same way.

.. code-block:: shell

   ironplcc check --allow-constant-initializer-expressions main.st

.. |flag| replace:: ``--allow-constant-initializer-expressions``
.. include:: /includes/enabled-by-flag.rst

.. playground::
   :allows: constant-initializer-expressions

   PROGRAM main
       VAR CONSTANT
           LIMIT : INT := 100;
           NAME : STRING := 'pump';
       END_VAR
       VAR
           threshold : INT := LIMIT;
           label : STRING[20] := NAME;
       END_VAR

       threshold := threshold - 1;
   END_PROGRAM

The constant's declared type must be compatible with the variable's type by
the same rule as an assignment: an ``INT`` constant may initialize a
``DINT`` variable, but a ``UDINT`` constant initializing an ``INT``
variable is reported as :doc:`P4022 </reference/compiler/problems/P4022>`.
A name that is not a constant is reported as
:doc:`P4038 </reference/compiler/problems/P4038>`, and this form without
the flag as :doc:`P4037 </reference/compiler/problems/P4037>`.

See Also
--------

- :doc:`declarations` — variable declaration syntax
- :doc:`/reference/language/data-types/index` — default values for each type
