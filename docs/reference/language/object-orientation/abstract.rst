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
     - Parsed and analyzed: a variable declared with the type of an
       ``ABSTRACT`` function block reports
       :doc:`P4045 </reference/compiler/problems/P4045>`, while a concrete
       type that :doc:`extends <extends>` it instantiates normally. The
       check covers a direct declaration; it does not yet reject an
       ``ABSTRACT`` type used as the element type of an array. IronPLC
       does not yet execute an ``ABSTRACT`` function block, so declaring
       one also reports
       :doc:`P9999 </reference/compiler/problems/P9999>`. Enable with
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

.. playground::
   :dialect: iec61131-3-ed3

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

   PROGRAM main
       VAR
           axis : FB_LinearAxis;
       END_VAR
   END_PROGRAM

``FB_BaseAxis`` cannot be instantiated on its own; ``FB_LinearAxis`` extends
it and can be, so ``axis`` is a valid declaration.

Naming the abstract type in the declaration instead reports
:doc:`P4045 </reference/compiler/problems/P4045>`:

.. playground::
   :dialect: iec61131-3-ed3

   FUNCTION_BLOCK ABSTRACT FB_BaseAxis
   END_FUNCTION_BLOCK

   PROGRAM main
       VAR
           axis : FB_BaseAxis;    (* P4045: FB_BaseAxis is ABSTRACT *)
       END_VAR
   END_PROGRAM

See Also
--------

- :doc:`extends` — derive from a base type
- :doc:`implements` — provide the methods declared by an interface
- :doc:`interface` — declare an interface
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
- :doc:`P4045 </reference/compiler/problems/P4045>` — the error reported for
  instantiating an abstract type
