===========================
Object Oriented Programming
===========================

IEC 61131-3 Edition 3 (2013) added **object-oriented programming** to the
standard. It builds on the function block — the language's existing unit of
reusable, stateful behavior — and adds three ideas: **inheritance**,
**interfaces**, and **abstract** types. This page explains those concepts and
the terminology the standard uses for them.

.. note::

   The object-oriented syntax described here is recognized only when it is
   enabled; otherwise the keywords are ordinary identifiers. See
   :doc:`enabling-dialects-and-features` for the flag and dialects reference.
   IronPLC currently *parses* this syntax but does not yet analyze or execute
   it — a program that uses it reports
   :doc:`P9999 </reference/compiler/problems/P9999>`. This page describes the
   language concepts; it is not a statement of what IronPLC executes today.

Why object-oriented programming?
================================

A :doc:`function block </reference/language/pous/function-block>` already
bundles data and behavior: it has internal state and runs code that acts on
that state. Object-oriented programming extends this in two directions. First,
it lets one function block type build on another instead of copying it —
so shared behavior is written once and reused. Second, it lets code work with
an instance through a *contract* rather than a specific type — so the same
code can drive many different implementations. The standard expresses these
with a small set of keywords.

Inheritance
===========

**Inheritance** lets a function block type build on an existing one. A type is
**derived** from a **base** type using the
:doc:`EXTENDS </reference/language/object-orientation/extends>` keyword. The
derived type inherits the base type's variables and methods, and it can add
its own or refine what it inherited.

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

Here ``FB_AdvancedMotor`` is the derived type and ``FB_Motor`` is its base
type. A function block type has at most one direct base type — the standard
provides single inheritance for function block types. An instance of a derived
type *is a* kind of its base type, so it can be used wherever the base type is
expected.

Hiding inherited names
----------------------

A derived type can declare a variable whose name is already used by an
inherited variable. The declaration in the derived type **hides** (or
*shadows*) the inherited one: within the derived type's own body the name
refers to the member declared there, not to the one it inherited. The
inherited member is not removed — it still exists on every instance and is
reached through the base type.

.. code-block::

   FUNCTION_BLOCK FB_Base
       VAR
           state : INT;
       END_VAR
   END_FUNCTION_BLOCK

   FUNCTION_BLOCK FB_Derived EXTENDS FB_Base
       VAR
           state : BOOL;
       END_VAR
   END_FUNCTION_BLOCK

Inside ``FB_Derived``, ``state`` names the ``BOOL`` declared there; the
inherited ``INT`` is hidden but still present on the instance.

Hiding is different from declaring the same name twice in one place. Two
declarations of the same name in a single scope — for example two variables in
one ``VAR`` block — are a duplicate and are rejected
(:doc:`P4014 </reference/compiler/problems/P4014>`). Hiding involves two
*different* scopes, the base type and the derived type, so the name is
resolved by choosing the nearer declaration rather than reported as an error.

Interfaces
==========

An **interface** is a named contract: a set of **method** signatures — the
name, parameters, and return type of each method — with no implementation. An
interface is declared with
:doc:`INTERFACE </reference/language/object-orientation/interface>` and closed
with ``END_INTERFACE``. A function block type states that it fulfils the
contract with the
:doc:`IMPLEMENTS </reference/language/object-orientation/implements>` keyword,
which obliges it to supply a body for every method the interface declares.

.. code-block::

   INTERFACE I_Drivable
   END_INTERFACE

   FUNCTION_BLOCK FB_Motor IMPLEMENTS I_Drivable
       VAR
           running : BOOL;
       END_VAR
   END_FUNCTION_BLOCK

Unlike inheritance, interfaces are not limited to one: a function block type
may implement several interfaces, and interfaces themselves may
:doc:`extend </reference/language/object-orientation/extends>` other
interfaces to combine their contracts. Because any implementing type satisfies
the same contract, code written against an interface can operate on instances
of unrelated types — this is **polymorphism**, and selecting the right method
implementation for the actual instance at run time is called **dynamic
binding** (dynamic dispatch).

Interfaces and inheritance are complementary. Inheritance shares an
*implementation* down a single line of descent; an interface shares a
*contract* across any number of otherwise unrelated types. A declaration can
use both at once — extend a base type and implement one or more interfaces.

Abstract types
==============

A type marked
:doc:`ABSTRACT </reference/language/object-orientation/abstract>` is one that
is intended to be extended rather than instantiated on its own. An abstract
function block type defines the shared shape of a family of types — common
variables and method signatures — while leaving some behavior for derived
types to complete. Because it is incomplete, an instance of an abstract type
cannot be created directly; only a concrete type that extends it can be
instantiated.

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

``FB_BaseAxis`` establishes what every axis has in common; ``FB_LinearAxis``
extends it into a concrete type that can be instantiated.

Terminology
===========

.. list-table::
   :header-rows: 1
   :widths: 25 75

   * - Term
     - Meaning
   * - Base type
     - The type another type is derived from.
   * - Derived type
     - A type that extends a base type and inherits its members.
   * - Inheritance
     - Deriving one type from another so it reuses the base type's members.
   * - Hiding (shadowing)
     - A member declared in a derived type takes precedence, within that type,
       over an inherited member of the same name.
   * - Interface
     - A named set of method signatures with no implementation.
   * - Implement
     - Supply a body for every method an interface declares.
   * - Abstract type
     - A type that cannot be instantiated directly and is meant to be
       extended.
   * - Polymorphism
     - Using instances of different types through a shared base type or
       interface.
   * - Dynamic binding
     - Choosing the method implementation for the actual instance at run
       time.

Beyond ``EXTENDS``, ``IMPLEMENTS``, ``ABSTRACT``, and ``INTERFACE``, Edition 3
also defines ``METHOD``, ``PROPERTY``, ``OVERRIDE``, ``FINAL``, ``THIS``, and
``SUPER`` for writing and refining methods. IronPLC does not parse those yet.

See Also
========

- :doc:`/reference/language/object-orientation/index` — the object-oriented
  keywords
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
- :doc:`/reference/language/variables/scope` — variable scope and resolution
- :doc:`/reference/compiler/problems/P4014` — duplicate name in a single scope
- :doc:`enabling-dialects-and-features` — enabling the object-oriented syntax
- :doc:`/reference/compiler/problems/P9999` — the diagnostic reported for
  not-yet-supported object-oriented constructs

References
==========

* IEC 61131-3:2013 — object-oriented extensions (``EXTENDS``, ``SUPER``,
  class and function block inheritance)
* `CODESYS: Extension of a Function Block (EXTENDS) <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_extending_function_block.html>`_
* `CODESYS: Shadowing Rules <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_shadowing_rules.html>`_
* `CODESYS: SUPER pointer <https://content.helpme-codesys.com/en/CODESYS%20Development%20System/_cds_pointer_super.html>`_
* `CODESYS: Static Analysis SA0013 (Declarations with the same variable name) <https://content.helpme-codesys.com/en/CODESYS%20Static%20Analysis/_san_rule_sa0013.html>`_
* `PLCCoder: IEC 61131-3 — The hiding attributes <https://www.plccoder.com/the-hiding-attributes/>`_
* `Stefan Henneken: IEC 61131-3 — Methods, Properties and Inheritance <https://stefanhenneken.net/2017/04/23/iec-61131-3-methods-properties-and-inheritance/>`_
