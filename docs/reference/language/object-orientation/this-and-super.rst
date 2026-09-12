==============
THIS and SUPER
==============

``THIS`` and ``SUPER`` name the function block instance a method is running
on. ``THIS^`` is that instance itself, which is how a method reaches a
variable of the function block that one of its own parameters or locals
hides. ``SUPER^`` is the same instance seen as its
:doc:`base type <extends>`, which is how a derived type calls the method it
overrode. Both are pointers, so both are written with the dereference
operator ``^``.

Neither is a way to reach a second variable of an inherited name: a derived
type cannot redeclare a variable it inherits
(:doc:`P4044 </reference/compiler/problems/P4044>`).

.. |keyword| replace:: ``THIS`` and ``SUPER``
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

The dereference operator is required — ``THIS`` and ``SUPER`` are pointers,
so ``THIS.count`` is not valid where ``THIS^.count`` is. Whitespace between
the keyword and the ``^`` is accepted:

.. code-block:: bnf

   THIS ^ . member
   SUPER ^ . member

A dereferenced reference is used wherever a variable is: read from, assigned
to, subscripted, or called.

Example
-------

.. code-block::

   FUNCTION_BLOCK FB_Motor
       VAR
           speed : INT;
       END_VAR

       METHOD Stop
           THIS^.speed := 0;
       END_METHOD
   END_FUNCTION_BLOCK

   FUNCTION_BLOCK FB_LoggingMotor EXTENDS FB_Motor
       METHOD Stop
           SUPER^.Stop();
       END_METHOD
   END_FUNCTION_BLOCK

``FB_LoggingMotor`` declares its own ``Stop``, which overrides the one it
inherits. ``SUPER^.Stop()`` calls the overridden base implementation;
writing ``Stop()`` there would call itself.

``THIS^`` is most useful when a local name hides a member of the instance —
for example a method parameter named after a variable of the function block.
``THIS^.speed`` then names the function block's variable, and ``speed`` alone
names the parameter.

See Also
--------

- :doc:`extends` — derive from a base type
- :doc:`method` — declare a method on a function block type
- :doc:`abstract` — mark a type as not directly instantiable
- :doc:`interface` — declare an interface
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
