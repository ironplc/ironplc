=========
INTERFACE
=========

``INTERFACE`` declares an interface: a named set of method signatures with
no implementation. A function block type that :doc:`implements <implements>`
the interface promises to supply a body for each of those methods, which lets
instances of unrelated types be used through the same interface. An interface
declaration is terminated by ``END_INTERFACE``. Interfaces are part of the
object-oriented programming introduced in IEC 61131-3 Edition 3.

.. |keyword| replace:: ``INTERFACE``
.. |flag| replace:: ``--allow-fb-inheritance``
.. include:: /includes/oop-keyword-flag.rst

.. note::

   ``END_INTERFACE`` is the closing keyword of an interface declaration and
   is gated by the same flag. Like ``INTERFACE``, it is an ordinary
   identifier when the flag is not enabled.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Edition 3 (object-oriented programming)
   * - **Support**
     - Parsed and analyzed. Calling a method through an interface, and
       compiling a program with a variable of an interface type, are not
       yet supported (:doc:`P9999 </reference/compiler/problems/P9999>`).
       Enable with ``--allow-fb-inheritance``; see
       :doc:`/explanation/enabling-dialects-and-features`.

Syntax
------

.. code-block:: bnf

   INTERFACE interface_name [EXTENDS base_interface {, base_interface}]
       { method_prototype | property_prototype }
   END_INTERFACE

   method_prototype ::=
       METHOD method_name [: return_type]
           { VAR_INPUT ... END_VAR | VAR_OUTPUT ... END_VAR | VAR_IN_OUT ... END_VAR }
       END_METHOD

   property_prototype ::=
       PROPERTY property_name : property_type
           [GET END_GET]
           [SET END_SET]
       END_PROPERTY

An interface lists the signatures of its :doc:`methods <method>` and
:doc:`properties <property>`, in any order, but no bodies: a method prototype
has only input, output and in-out variables, and a property prototype says
which accessors exist. An interface may :doc:`extend <extends>` one or more
base interfaces, inheriting their signatures.

Example
-------

.. code-block::

   INTERFACE I_Drivable
       METHOD Start : BOOL
           VAR_INPUT
               speed : INT;
           END_VAR
       END_METHOD
       METHOD Stop
       END_METHOD
       PROPERTY Running : BOOL
           GET END_GET
       END_PROPERTY
   END_INTERFACE

   INTERFACE I_PoweredDrivable EXTENDS I_Drivable
   END_INTERFACE

   FUNCTION_BLOCK FB_Motor IMPLEMENTS I_Drivable
       VAR
           _running : BOOL;
       END_VAR
       METHOD Start : BOOL
           VAR_INPUT
               speed : INT;
           END_VAR
           _running := speed > 0;
           Start := _running;
       END_METHOD
       METHOD Stop
           _running := FALSE;
       END_METHOD
       PROPERTY Running : BOOL
           GET
               Running := _running;
           END_GET
       END_PROPERTY
   END_FUNCTION_BLOCK

Interface variables
-------------------

A variable can have an interface type. It refers to an instance of a
function block type that implements the interface, directly, through a base
type it :doc:`extends <extends>`, or through an interface that extends the
one required. A variable of an interface type starts out referring to
nothing and cannot have an initial value. Assigning ``0`` makes it refer to
nothing again, and comparing it with ``0`` tells whether it refers to an
instance.

.. code-block::

   PROGRAM main
       VAR
           motor : FB_Motor;
           drive : I_Drivable;
       END_VAR
       drive := motor;
   END_PROGRAM

Assigning a value that does not implement the interface, or passing one as
an argument to an input of an interface type, is
:doc:`P4066 </reference/compiler/problems/P4066>`.

.. note::

   IronPLC does not yet check that a function block provides the methods
   and properties of the interfaces it implements. Calling a method through
   an interface variable (``drive.Start(10)``) needs dynamic dispatch and
   is reported as not yet supported.

See Also
--------

- :doc:`implements` — provide the methods declared by an interface
- :doc:`method` — declare a method on a function block type
- :doc:`property` — declare a property on a function block type
- :doc:`extends` — derive an interface or function block from a base
- :doc:`abstract` — mark a function block type as not directly instantiable
- :doc:`/explanation/object-orientation` — inheritance, interfaces, and
  abstract types explained
