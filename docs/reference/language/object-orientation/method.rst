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
     - Parsed, analyzed, compiled and executed for a method declared on
       the instance's own type: a declaration is checked, and a call is
       resolved against the declared type of the instance, walking the
       ``EXTENDS`` chain to find the method. A call that the chain
       resolves to an inherited method is accepted by analysis but not
       yet compiled (see :ref:`method-limitations`). Enable with
       ``--allow-fb-inheritance``; see
       :doc:`/explanation/enabling-dialects-and-features`.

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

A method is called on an instance using the same dot notation as a
structure member. Arguments are written positionally or by name, exactly as
for a function block invocation. The call can be a statement, which discards
any return value, or part of an expression, where it evaluates to the
method's return value:

.. code-block:: bnf

   instance_name.method_name(argument_list);
   variable := instance_name.method_name(argument_list);

A method without a return type has no value to use, so calling it in an
expression reports :doc:`P4057 </reference/compiler/problems/P4057>`.

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

       METHOD IsRunning : BOOL
           IsRunning := speed > 0;
       END_METHOD
   END_FUNCTION_BLOCK

``SetSpeed`` takes one parameter; ``Stop`` takes none. Both act on the
``speed`` variable of the instance they are called on. ``IsRunning`` returns
a value, so it can be used in a condition:

.. code-block::

   PROGRAM Main
       VAR
           motor : FB_Motor;
       END_VAR
       motor.SetSpeed(newSpeed := 1200);
       IF motor.IsRunning() THEN
           motor.Stop();
       END_IF;
   END_PROGRAM

.. _method-limitations:

Current limitations
-------------------

A method with a ``STRING`` or ``WSTRING`` return type reports
:doc:`P9999 </reference/compiler/problems/P9999>` when compiled.

A call to a method the instance's type inherits through
:doc:`EXTENDS <extends>` passes analysis but reports
:doc:`P9999 </reference/compiler/problems/P9999>` when compiled. Code
generation does not yet give a derived type storage for what it inherits,
so there is nothing for the inherited body to run against. Declare the
method on the type being called, or call it on an instance of the type
that declares it, until inherited storage is compiled.

See Also
--------

- :doc:`extends` — derive from a base type and inherit its methods
- :doc:`this-and-super` — the instance a method runs on, and its base type
- :doc:`interface` — declare a set of method signatures
- :doc:`implements` — provide the methods declared by an interface
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
- :doc:`/reference/language/pous/function-block` — the ``FUNCTION_BLOCK`` unit
