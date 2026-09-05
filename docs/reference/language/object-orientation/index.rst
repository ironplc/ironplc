===========================
Object Oriented Programming
===========================

IEC 61131-3 Edition 3 (2013) added object-oriented programming to the
standard. A function block type can declare **methods**, it can be
**derived** from another by **extension**, one or more **interfaces** can
describe a shared set of method signatures, and a type can be declared
**abstract** when it is meant to be extended rather than instantiated
directly.

This section documents the keywords that introduce these features. For the
concepts behind them — inheritance, interfaces, and abstract types — see
:doc:`/explanation/object-orientation`.

.. note::

   These keywords are recognized only when object-oriented syntax is
   enabled; otherwise each word is an ordinary identifier. See
   :doc:`/explanation/enabling-dialects-and-features` for the flag and
   dialects reference.

Keywords
--------

.. list-table::
   :header-rows: 1
   :widths: 25 40 35

   * - Keyword
     - Description
     - Support
   * - :doc:`extends`
     - Derive a function block type or interface from a base type
     - Parsed and analyzed; inherited variables are not yet compiled
   * - :doc:`implements`
     - Declare that a function block type provides one or more interfaces
     - Parsed only
   * - :doc:`abstract`
     - Mark a function block type as not directly instantiable
     - Parsed only
   * - :doc:`interface`
     - Declare an interface — a named set of method signatures
     - Parsed only
   * - :doc:`method`
     - Declare a method on a function block type
     - Parsed and analyzed; calls are not yet compiled
   * - :doc:`this-and-super`
     - Refer to the instance a method is running on, or to its base type
     - Parsed only

"Parsed only" means the syntax is accepted and the names it introduces are
recognized, but the construct is not yet analyzed or executed — using it
reports :doc:`P9999 </reference/compiler/problems/P9999>`. Each keyword's
page states exactly what it supports today.

.. toctree::
   :maxdepth: 1
   :hidden:

   extends
   implements
   abstract
   interface
   method
   this-and-super
