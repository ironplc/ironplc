========
ABSTRACT
========

``ABSTRACT`` marks a function block type that is meant to be extended rather
than instantiated directly. An abstract type provides a common base — shared
variables and method signatures — that derived types complete. An instance
of an abstract type cannot be created; only a concrete type that
:doc:`extends <extends>` it can be. This is part of the object-oriented
programming introduced in IEC 61131-3 Edition 3.

.. |keyword| replace:: ``ABSTRACT``
.. |flag| replace:: ``--allow-fb-inheritance``
.. include:: /includes/oop-keyword-flag.rst

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Edition 3 (object-oriented programming)
   * - **Support**
     - Parsed only — not yet analyzed or executed
       (:doc:`P9999 </reference/compiler/problems/P9999>`). Enable with
       ``--allow-fb-inheritance``; see
       :doc:`/explanation/enabling-dialects-and-features`.

Syntax
------

``ABSTRACT`` appears between ``FUNCTION_BLOCK`` and the type name. It may be
combined with :doc:`EXTENDS <extends>` and :doc:`IMPLEMENTS <implements>`:

.. code-block:: bnf

   FUNCTION_BLOCK ABSTRACT fb_name [EXTENDS base_name] [IMPLEMENTS interface_name {, interface_name}]
       variable_declarations
       statement_list
   END_FUNCTION_BLOCK

Example
-------

.. code-block::

   FUNCTION_BLOCK ABSTRACT FB_BaseAxis
       VAR
           enabled : BOOL;
       END_VAR
   END_FUNCTION_BLOCK

   FUNCTION_BLOCK FB_LinearAxis EXTENDS FB_BaseAxis
       VAR
           position : REAL;
       END_VAR
   END_FUNCTION_BLOCK

``FB_BaseAxis`` cannot be instantiated on its own; ``FB_LinearAxis`` extends
it and can be.

See Also
--------

- :doc:`extends` — derive from a base type
- :doc:`implements` — provide the methods declared by an interface
- :doc:`interface` — declare an interface
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
