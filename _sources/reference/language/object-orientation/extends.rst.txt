=======
EXTENDS
=======

``EXTENDS`` derives a function block type from a single base type, so the
derived type **inherits** the base type's variables and
:doc:`methods <method>`. It also derives an interface from one or more base
interfaces. This is the inheritance mechanism introduced in
IEC 61131-3 Edition 3.

.. |keyword| replace:: ``EXTENDS``
.. |flag| replace:: ``--allow-fb-inheritance``
.. include:: /includes/oop-keyword-flag.rst

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Edition 3 (object-oriented programming)
   * - **Support**
     - On a function block, parsed and analyzed: an inherited variable
       resolves and type-checks in the derived type, a call to an instance
       passes inherited inputs and outputs as it does the derived type's own,
       a call resolves to a method declared anywhere up the ``EXTENDS``
       chain, and a derived type that redeclares an inherited variable
       reports :doc:`P4044 </reference/compiler/problems/P4044>`. Code
       generation does not yet give a derived type storage for what it
       inherits, so compiling a derived type reports
       :doc:`P9999 </reference/compiler/problems/P9999>` where it reads or
       writes an inherited variable, passes one in a call, or calls a method
       inherited from a type that declares variables. On an
       interface, parsed only. Enable with
       ``--allow-fb-inheritance``; see
       :doc:`/explanation/enabling-dialects-and-features`.

Syntax
------

On a function block declaration, ``EXTENDS`` names the single base type:

.. code-block:: bnf

   FUNCTION_BLOCK derived_name EXTENDS base_name
       variable_declarations
       statement_list
   END_FUNCTION_BLOCK

On an :doc:`interface` declaration, ``EXTENDS`` names one or more base
interfaces, separated by commas:

.. code-block:: bnf

   INTERFACE interface_name EXTENDS base_interface {, base_interface}
   END_INTERFACE

Example
-------

.. code-block::

   FUNCTION_BLOCK FB_Motor
       VAR
           running : BOOL;
       END_VAR
   END_FUNCTION_BLOCK

   FUNCTION_BLOCK FB_AdvancedMotor EXTENDS FB_Motor
       VAR
           speed : INT;
       END_VAR
   END_FUNCTION_BLOCK

``FB_AdvancedMotor`` is the derived type and ``FB_Motor`` is its base type.
A function block type extends at most one base type, and the base type must
be a declared function block type. A name that is not declared, or that names
another kind of type, reports :doc:`P4088 </reference/compiler/problems/P4088>`.

What a derived type inherits
----------------------------

A derived type has one set of variables: every variable its base type
declares, plus the ones it declares itself. It cannot declare a variable
with a name it inherits — that reports
:doc:`P4044 </reference/compiler/problems/P4044>`, whatever type the second
declaration gives and however far up the ``EXTENDS`` chain the first one
sits. Edition 3 gives a derived type no way to hold a second variable of an
inherited name: ``SUPER^`` selects the base type's *implementation* of a
method, not a second variable of the same name.

Methods work the other way round. A derived type may declare a method with
the name of one it inherits, which **overrides** it: a call on the derived
type runs the derived type's method, and that method reaches the one it
overrode through :doc:`SUPER^ <this-and-super>`. An override has the
signature of the method it overrides: the same return type and the same
``VAR_INPUT``, ``VAR_OUTPUT`` and ``VAR_IN_OUT`` variables, in the same order.
One that differs reports :doc:`P4089 </reference/compiler/problems/P4089>`.
A property overrides an inherited property of its name in the same way, and
has the type of the property it overrides; one of another type reports
:doc:`P4090 </reference/compiler/problems/P4090>`.

See Also
--------

- :doc:`method` — declare a method on a function block type
- :doc:`implements` — provide the methods declared by an interface
- :doc:`abstract` — mark a base type as not directly instantiable
- :doc:`interface` — declare an interface
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
