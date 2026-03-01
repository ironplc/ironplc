=============
Function Call
=============

Functions and function blocks are invoked using call syntax that passes
arguments and receives results.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.2
   * - **Support**
     - Partial

Syntax
------

**Function call (expression)**

.. code-block:: bnf

   result := function_name ( argument_list ) ;

**Function block instance call (statement)**

.. code-block:: bnf

   instance_name ( input_assignments ) ;
   output_value := instance_name.output_name ;

Description
-----------

Functions are called within expressions and return a value. Function blocks
are called as statements on a previously declared instance variable; outputs
are accessed by qualifying the instance name with the output name.

**Positional arguments** pass values in the order of the input parameter
declarations:

.. code-block::

   result := MyFunc(10, 20);

**Named (formal) arguments** explicitly associate values with parameter names
using the ``:=`` notation:

.. code-block::

   result := MyFunc(x := 10, y := 20);

Positional and named arguments must not be mixed in a single call.

**Function block calls** use named arguments for inputs. After the call,
outputs are read from the instance:

.. code-block::

   my_timer(IN := start_signal, PT := T#5s);
   elapsed := my_timer.ET;
   done := my_timer.Q;

Example
-------

.. code-block::

   FUNCTION Add : DINT
       VAR_INPUT
           a : DINT;
           b : DINT;
       END_VAR

       Add := a + b;
   END_FUNCTION

   PROGRAM main
       VAR
           result : DINT;
       END_VAR

       (* Positional call *)
       result := Add(3, 4);

       (* Named call *)
       result := Add(a := 10, b := 20);
   END_PROGRAM

Related Problem Codes
---------------------

- :doc:`/reference/compiler/problems/P4001` — Mixed named and positional arguments
- :doc:`/reference/compiler/problems/P4002` — Missing required input parameter
- :doc:`/reference/compiler/problems/P4003` — Invocation requires formal (named) arguments
- :doc:`/reference/compiler/problems/P4004` — Undefined output on function invocation
- :doc:`/reference/compiler/problems/P4017` — Undeclared function call
- :doc:`/reference/compiler/problems/P4018` — Wrong argument count

See Also
--------

- :doc:`/reference/language/pous/function` — function definition
- :doc:`/reference/language/pous/function-block` — function block definition
- :doc:`assignment` — storing return values
