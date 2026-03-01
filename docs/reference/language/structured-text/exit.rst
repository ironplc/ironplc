====
EXIT
====

The ``EXIT`` statement terminates execution of the innermost enclosing loop.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3.2.4
   * - **Support**
     - Supported

Syntax
------

.. code-block:: bnf

   EXIT ;

Description
-----------

``EXIT`` immediately breaks out of the innermost ``FOR``, ``WHILE``, or
``REPEAT`` loop. Execution continues with the first statement after the loop's
closing keyword (``END_FOR``, ``END_WHILE``, or ``END_REPEAT``).

If ``EXIT`` appears inside nested loops, only the innermost loop is
terminated.

Example
-------

.. code-block::

   PROGRAM main
       VAR
           i : INT;
           found : BOOL := FALSE;
           data : INT := 42;
       END_VAR

       FOR i := 1 TO 100 DO
           IF i = data THEN
               found := TRUE;
               EXIT;
           END_IF;
       END_FOR;
   END_PROGRAM

See Also
--------

- :doc:`for` — counted loop
- :doc:`while` — pre-tested loop
- :doc:`repeat` — post-tested loop
- :doc:`return` — early exit from POU
