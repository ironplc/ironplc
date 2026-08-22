===========================
Object Oriented Programming
===========================

IEC 61131-3 Edition 3 (2013) added object-oriented programming to the
standard. A function block type can be **derived** from another by
**extension**, one or more **interfaces** can describe a shared set of
method signatures, and a type can be declared **abstract** when it is meant
to be extended rather than instantiated directly.

This section documents the keywords that introduce these features. For the
concepts behind them — inheritance, interfaces, and abstract types — see
:doc:`/explanation/object-orientation`.

.. note::

   These keywords are recognized only when object-oriented syntax is
   enabled; otherwise each word is an ordinary identifier. See
   :doc:`/explanation/enabling-dialects-and-features` for the flag and
   dialects reference. IronPLC currently parses this syntax but does not yet
   perform semantic analysis or code generation for it — see
   :doc:`/reference/compiler/problems/P9999`.

Keywords
--------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Keyword
     - Description
   * - :doc:`extends`
     - Derive a function block type or interface from a base type
   * - :doc:`implements`
     - Declare that a function block type provides one or more interfaces
   * - :doc:`abstract`
     - Mark a function block type as not directly instantiable
   * - :doc:`interface`
     - Declare an interface — a named set of method signatures
   * - :doc:`this-and-super`
     - Refer to the instance a method is running on, or to its base type

.. toctree::
   :maxdepth: 1
   :hidden:

   extends
   implements
   abstract
   interface
   this-and-super
