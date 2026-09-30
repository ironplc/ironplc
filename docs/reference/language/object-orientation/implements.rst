==========
IMPLEMENTS
==========

``IMPLEMENTS`` declares that a function block type provides every method of
one or more :doc:`interfaces <interface>`. The type promises to supply an
implementation for each method signature the interface declares, which lets
instances of unrelated types be used through a common interface. This is
part of the object-oriented programming introduced in IEC 61131-3 Edition 3.

.. |keyword| replace:: ``IMPLEMENTS``
.. |flag| replace:: ``--allow-fb-inheritance``
.. include:: /includes/oop-keyword-flag.rst

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Edition 3 (object-oriented programming)
   * - **Support**
     - Parsed and analyzed: the function block must provide the
       interface's members (:doc:`P4067 </reference/compiler/problems/P4067>`,
       :doc:`P4068 </reference/compiler/problems/P4068>`), and an instance
       can be used where an interface it implements is required. Enable with
       ``--allow-fb-inheritance``; see
       :doc:`/explanation/enabling-dialects-and-features`.

Syntax
------

``IMPLEMENTS`` names one or more interfaces, separated by commas. It may
follow an :doc:`EXTENDS <extends>` clause on the same declaration:

.. code-block:: bnf

   FUNCTION_BLOCK fb_name [EXTENDS base_name]
       IMPLEMENTS interface_name {, interface_name}
       variable_declarations
       statement_list
   END_FUNCTION_BLOCK

Example
-------

.. code-block::

   INTERFACE I_Drivable
   END_INTERFACE

   FUNCTION_BLOCK FB_Motor IMPLEMENTS I_Drivable
       VAR
           running : BOOL;
       END_VAR
   END_FUNCTION_BLOCK

The function block must declare every method and property of the
interface, and of the interfaces it extends, with the same signature: a
method with the same return type and the same input, output and in-out
variables, in the same order, and a property with the same type and every
accessor the interface declares. A member inherited from a base type through
``EXTENDS`` counts.

A type may implement several interfaces at once:

.. code-block::

   FUNCTION_BLOCK FB_Actuator IMPLEMENTS I_Hydraulics, I_Brake
   END_FUNCTION_BLOCK

See Also
--------

- :doc:`interface` — declare an interface
- :doc:`method` — declare a method on a function block type
- :doc:`extends` — derive from a base type
- :doc:`abstract` — mark a type as not directly instantiable
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
