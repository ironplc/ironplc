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

Explicit Values
---------------

A member can be given its own value instead of the one its position implies.
Members that follow continue from the value before them, so ``Type_ANY`` below
is 1 and ``Type_BOOL`` is 2:

.. code-block::

   TYPE
       E_AssertionType : (Type_UNDEFINED := 0, Type_ANY, Type_BOOL);
   END_TYPE

This is IEC 61131-3:2013 (Edition 3) syntax, not Edition 2, so the default
strict Edition 2 dialect rejects it with
:doc:`/reference/compiler/problems/P4055`. Select a dialect that includes it
or pass ``--allow-enum-explicit-values`` — see
:doc:`/explanation/enabling-dialects-and-features`.

Values are not checked for uniqueness: ``(A := 1, B := 1)`` gives two names
for the same value and is accepted. Only the *names* must differ.

Base Type
---------

A declaration can name the elementary type the members are stored in:

.. code-block::

   TYPE
       Color : (Red, Green, Blue) INT;
   END_TYPE

Without it, IronPLC picks the smallest type that holds every member's value.
The suffix is a CODESYS/TwinCAT extension beyond IEC 61131-3 rather than
standard syntax; IronPLC currently accepts it under every dialect, including
strict Edition 2.

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P2003` — Duplicate enumeration value
- :doc:`/reference/compiler/problems/P4055` — Explicit enumeration member
  value requires a dialect or flag

See Also
--------

- :doc:`subrange-types` — restrict an integer to a range
