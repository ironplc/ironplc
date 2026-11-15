======
RETURN
======

The ``RETURN`` statement causes an early exit from the current program
organization unit.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.5
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   RETURN ;

Description
-----------

``RETURN`` terminates execution of the current program, function, or function
block. In a function, the return value is the last value assigned to the
function name before ``RETURN`` executes. In a program or function block,
execution resumes at the caller.

Example
-------

.. playground::

   FUNCTION Divide : DINT
       VAR_INPUT
           numerator : DINT;
           denominator : DINT;
       END_VAR

       IF denominator = 0 THEN
           Divide := 0;
           RETURN;
       END_IF;

       Divide := numerator / denominator;
   END_FUNCTION

   PROGRAM main
       VAR
           safe_result : DINT;
           normal_result : DINT;
       END_VAR

       safe_result := Divide(10, 0);
       normal_result := Divide(10, 3);
   END_PROGRAM

Conditional Return (Language Extension)
---------------------------------------

.. include:: ../../../includes/requires-dialect-extension.rst

.. |flag| replace:: ``--allow-conditional-return``
.. include:: /includes/enabled-by-flag.rst

.. code-block:: bnf

   RETURN ( expression ) ;

``RETURN(condition)`` returns only when ``condition`` is ``TRUE``. When it is
``FALSE``, execution goes on with the next statement. It means the same as:

.. code-block::

   IF condition THEN
       RETURN;
   END_IF;

The condition must be a ``BOOL``; another type is an error
(:doc:`P4072 </reference/compiler/problems/P4072>`). The condition is never
the return value of a function, even a ``BOOL`` function: the function
returns the last value assigned to its name, as with a plain ``RETURN``.

Without the flag, ``RETURN(condition)`` is an error
(:doc:`P4098 </reference/compiler/problems/P4098>`).

.. playground::
   :dialect: twincat

   FUNCTION Divide : DINT
       VAR_INPUT
           numerator : DINT;
           denominator : DINT;
       END_VAR

       Divide := 0;
       RETURN(denominator = 0);
       Divide := numerator / denominator;
   END_FUNCTION

   PROGRAM main
       VAR
           safe_result : DINT;
           normal_result : DINT;
       END_VAR

       safe_result := Divide(10, 0);
       normal_result := Divide(10, 3);
   END_PROGRAM

See Also
--------

- :doc:`exit` — break from innermost loop
- :doc:`/reference/language/pous/function` — function definition
- :doc:`/reference/language/pous/function-block` — function block definition
