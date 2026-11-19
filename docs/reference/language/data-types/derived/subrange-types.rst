===============
Subrange Types
===============

A subrange type restricts an integer type to a specified range of values.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 2.3.3.1
   * - **Support**
     - Parsed, analyzed, compiled and executed. The range is checked at
       compile time only

Syntax
------

.. code-block:: bnf

   TYPE
       type_name : base_type ( lower_bound .. upper_bound ) [ := initial_value ] ;
   END_TYPE

The base type must be an integer type (``SINT``, ``INT``, ``DINT``,
``LINT``, ``USINT``, ``UINT``, ``UDINT``, or ``ULINT``).

A variable of a subrange type without an initial value starts at the type's
initial value, or else at the lower bound.

Example
-------

.. playground::

   TYPE
       Percent : INT (0 .. 100);
       Byte_Range : USINT (0 .. 255);
   END_TYPE

   PROGRAM main
       VAR
           level : Percent := 50;
       END_VAR

       level := level + 10;
   END_PROGRAM

Inline Subrange
---------------

A variable can declare its subrange in place, without a named type, in
``VAR``, ``VAR_INPUT``, ``VAR_OUTPUT``, ``VAR_TEMP``, ``VAR_GLOBAL`` and
``VAR_EXTERNAL`` blocks, in a located declaration and in a structure field.
An initial value is optional, except in ``VAR_EXTERNAL`` and
``VAR_IN_OUT``, which take none:

.. playground::

   PROGRAM main
       VAR
           channel : INT (0 .. 15);         (* starts at 0 *)
           level : INT (1 .. 10) := 5;
       END_VAR

       channel := level + 3;
   END_PROGRAM

A function's ``VAR_INPUT`` with an inline subrange is not yet supported:
a call to the function reports the argument as not matching any input.
Declare a named subrange type for a function input.

A ``VAR_EXTERNAL`` declaration with an inline subrange matches a
``VAR_GLOBAL`` declared with the same base type and bounds, and does not
match one declared with a named subrange type.

Range Checking
--------------

The compiler checks every constant that it assigns to a subrange variable:
an initial value, an assigned literal or a literal call argument outside the
range is error :doc:`/reference/compiler/problems/P2026`.

The range is not checked at run time. A value computed at run time is
stored as the base type stores it, even when it is outside the range:

.. playground::

   PROGRAM main
       VAR
           level : INT (0 .. 100) := 90;
       END_VAR

       level := level + 20;   (* level = 110 *)
   END_PROGRAM

Constant Bounds (Language Extension)
------------------------------------

.. include:: ../../../../includes/requires-dialect-extension.rst

With the ``--allow-constant-type-params`` flag, you can
use global constants for the subrange bounds:

.. code-block::

   VAR_GLOBAL CONSTANT
       MIN_PERCENT : INT := 0;
       MAX_PERCENT : INT := 100;
   END_VAR

   TYPE
       Percent : INT (MIN_PERCENT .. MAX_PERCENT);
   END_TYPE

Radix Bounds (Language Extension)
---------------------------------

.. include:: ../../../../includes/requires-dialect-extension.rst

With the ``--allow-radix-subrange-bounds`` flag, a bound can be written in
hex, binary, or octal:

.. code-block::

   TYPE
       Byte_Range : INT (16#00 .. 16#FF);
   END_TYPE

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P2026` — Constant outside the subrange
- :doc:`/reference/compiler/problems/P4096` — Radix bound without the flag

See Also
--------

- :doc:`enumerated-types` — named set of values
- :doc:`../elementary/int` — base integer type
