================
Enumerated Types
================

An enumerated type defines a named set of values.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.3.3.1
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   TYPE
       type_name : ( value1, value2, ... ) ;
   END_TYPE

Example
-------

.. playground::

   TYPE
       TrafficLight : (Red, Yellow, Green);
   END_TYPE

   PROGRAM main
       VAR
           state : TrafficLight := Red;
       END_VAR

       IF state = Green THEN
           state := Yellow;
       END_IF;
   END_PROGRAM

Member names must be unique within the type. The members take consecutive
values starting at zero, so ``Red`` is 0, ``Yellow`` is 1 and ``Green`` is 2.

Inline Enumerations
-------------------

A variable can list its values in place of a type name. The values number
the same way as in a ``TYPE`` declaration, and a variable without an initial
value starts at the first one:

.. playground::

   PROGRAM main
       VAR
           mode : (Idle, Running, Stopped) := Running;
           count : DINT;
       END_VAR

       CASE mode OF
           Idle: count := 0;
           Running: count := count + 1;
           Stopped: mode := Idle;
       END_CASE;
   END_PROGRAM

Each such declaration declares a type of its own, even when two declarations
list the same values. The type has no name, so debugging output such as
``ironplcvm run --dump-vars`` shows the variable's value as ``RUNNING (1)``
and describes its type by a generated name.

Values Shared by Two Enumerations
---------------------------------

Two enumerations may declare the same value name. A value written without its
type takes the type of where it is used: the variable it is assigned to, the
other operand of a comparison, the selector of a ``CASE``, the input of a
function block it is passed to, or the variable it initializes:

.. playground::

   TYPE
       Color : (Red, Green);
       Light : (Off, Green);
   END_TYPE

   PROGRAM main
       VAR
           shade : Color := Green;  (* 1, the Green of Color *)
           lamp : Light;
       END_VAR

       lamp := Green;               (* 1, the Green of Light *)
       IF shade = Green THEN        (* the Green of Color *)
           lamp := Off;
       END_IF;
   END_PROGRAM

Where nothing says which enumeration is meant, such as an assignment to a
``DINT``, the value is ambiguous and the compiler reports
:doc:`/reference/compiler/problems/P2043`.

Explicit Values
---------------

.. include:: ../../../../includes/requires-edition3.rst

A member can be given its own value instead of the one its position implies.
Members that follow continue from the value before them, so ``Type_ANY`` below
is 1 and ``Type_BOOL`` is 2:

.. playground::
   :allows: enum-explicit-values

   TYPE
       E_AssertionType : (Type_UNDEFINED := 0, Type_ANY, Type_BOOL);
   END_TYPE

   PROGRAM main
       VAR
           kind : E_AssertionType := Type_ANY;
       END_VAR

       IF kind = Type_ANY THEN
           kind := Type_BOOL;
       END_IF;
   END_PROGRAM

Values are not checked for uniqueness: ``(A := 1, B := 1)`` gives two names
for the same value and is accepted. Only the *names* must differ.

Base Type (Language Extension)
------------------------------

.. include:: ../../../../includes/requires-dialect-extension.rst

A declaration can name the elementary type the members are stored in:

.. playground::
   :allows: enum-base-type

   TYPE
       Color : (Red, Green, Blue) INT;
   END_TYPE

   PROGRAM main
       VAR
           shade : Color := Blue;
       END_VAR

       IF shade = Blue THEN
           shade := Red;
       END_IF;
   END_PROGRAM

Without it, IronPLC picks the smallest type that holds every member's value.

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P2003` — Duplicate enumeration value
- :doc:`/reference/compiler/problems/P2006` — Value not defined in the
  enumeration
- :doc:`/reference/compiler/problems/P2043` — Enumerated value is ambiguous
- :doc:`/reference/compiler/problems/P4055` — Explicit enumeration member
  value requires a dialect or flag
- :doc:`/reference/compiler/problems/P4056` — Enumeration base-type suffix
  requires a dialect or flag

See Also
--------

- :doc:`subrange-types` — restrict an integer to a range
