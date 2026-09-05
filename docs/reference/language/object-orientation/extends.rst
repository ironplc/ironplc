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
     - Parsed and analyzed: an inherited variable resolves and type-checks
       in the derived type. Code generation does not yet give a derived
       type storage for what it inherits, so reading or writing an
       inherited variable reports
       :doc:`P4007 </reference/compiler/problems/P4007>` when compiled.
       Enable with ``--allow-fb-inheritance``; see
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
A function block type extends at most one base type.

See Also
--------

- :doc:`method` — declare a method on a function block type
- :doc:`implements` — provide the methods declared by an interface
- :doc:`abstract` — mark a base type as not directly instantiable
- :doc:`interface` — declare an interface
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
