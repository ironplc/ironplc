======
METHOD
======

``METHOD`` declares a named operation on a function block type. A method has
its own parameters and local variables, an optional return type, and a body,
and it runs against the variables of the instance it is called on. Methods
are what an :doc:`interface` describes and what a derived type inherits
through :doc:`EXTENDS <extends>`. A method declaration is terminated by
``END_METHOD``. Methods are part of the object-oriented programming
introduced in IEC 61131-3 Edition 3.

.. |keyword| replace:: ``METHOD``
.. |flag| replace:: ``--allow-fb-inheritance``
.. include:: /includes/oop-keyword-flag.rst

.. note::

   ``END_METHOD`` is the closing keyword of a method declaration and is
   gated by the same flag. Like ``METHOD``, it is an ordinary identifier
   when the flag is not enabled.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Edition 3 (object-oriented programming)
   * - **Support**
     - Parsed and analyzed: a declaration is checked, and a call is
       resolved against the declared type of the instance, walking the
       ``EXTENDS`` chain to find the method. Calls are not yet compiled or
       executed (:doc:`P9999 </reference/compiler/problems/P9999>`), and a
       return value cannot be produced or consumed yet (see
       :ref:`method-limitations`). Enable with ``--allow-fb-inheritance``;
       see :doc:`/explanation/enabling-dialects-and-features`.

Syntax
------

A method is declared inside the function block it belongs to, between the
function block's variable blocks and ``END_FUNCTION_BLOCK``. The return type
is optional — a method without one is called for its effect rather than its
value:

.. code-block:: bnf

   METHOD method_name [: return_type]
       variable_declarations
       statement_list
   END_METHOD

Parameters are declared the same way as on a
:doc:`function </reference/language/pous/function>`, with ``VAR_INPUT``,
``VAR_OUTPUT``, and ``VAR_IN_OUT`` blocks; ``VAR`` declares locals of the
method itself.

A method is called on an instance, as a statement, using the same dot
notation as a structure member. Arguments are written positionally or by
name, exactly as for a function block invocation:

.. code-block:: bnf

   instance_name.method_name(argument_list);

The method to call is chosen from the *declared* type of the instance: the
type's own methods first, then those of its ``EXTENDS`` base, and so on up
the chain.

Example
-------

.. code-block::

   FUNCTION_BLOCK FB_Motor
       VAR
           speed : INT;
       END_VAR

       METHOD SetSpeed
           VAR_INPUT
               newSpeed : INT;
           END_VAR
           speed := newSpeed;
       END_METHOD

       METHOD Stop
           speed := 0;
       END_METHOD
   END_FUNCTION_BLOCK

``SetSpeed`` takes one parameter; ``Stop`` takes none. Both act on the
``speed`` variable of the instance they are called on:

.. code-block::

   PROGRAM Main
       VAR
           motor : FB_Motor;
       END_VAR
       motor.SetSpeed(newSpeed := 1200);
       motor.Stop();
   END_PROGRAM

.. _method-limitations:

Current limitations
-------------------

A return type is accepted on a declaration, but nothing can be done with it
yet. A method body cannot assign to the method's own name the way a
:doc:`function </reference/language/pous/function>` body does — that reports
:doc:`P4007 </reference/compiler/problems/P4007>` — and a call is a
statement, not an expression, so ``x := instance.Method()`` is a syntax
error. Write methods without a return type until return values are
supported.

See Also
--------

- :doc:`extends` — derive from a base type and inherit its methods
- :doc:`this-and-super` — the instance a method runs on, and its base type
- :doc:`interface` — declare a set of method signatures
- :doc:`implements` — provide the methods declared by an interface
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
