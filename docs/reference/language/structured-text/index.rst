===============
Structured Text
===============

Structured Text (ST) is a high-level textual programming language defined by
IEC 61131-3. It resembles Pascal and provides statements for assignment,
selection, iteration, and function invocation.

.. list-table::
   :widths: 30 70

   * - **IEC 61131-3**
     - Section 3.3

Statements
----------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Statement
     - Description
   * - :doc:`assignment`
     - Assign a value to a variable
   * - :doc:`bit-access`
     - Access a single bit of an integer or bit-string variable
   * - :doc:`if`
     - Conditional branching
   * - :doc:`case`
     - Multi-way selection by integer value
   * - :doc:`for`
     - Counted loop
   * - :doc:`while`
     - Pre-tested loop
   * - :doc:`repeat`
     - Post-tested loop
   * - :doc:`exit`
     - Break from innermost loop
   * - :doc:`return`
     - Early exit from POU

Operators
---------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Category
     - Description
   * - :doc:`arithmetic-operators`
     - Addition, subtraction, multiplication, division, modulo, power
   * - :doc:`comparison-operators`
     - Equality, inequality, less than, greater than
   * - :doc:`logical-operators`
     - AND, OR, XOR, NOT

Function Calls
--------------

.. list-table::
   :header-rows: 1
   :widths: 30 70

   * - Topic
     - Description
   * - :doc:`function-call`
     - Calling functions and function block instances

Operator Precedence
-------------------

Operators are listed from highest to lowest precedence.

.. list-table::
   :header-rows: 1
   :widths: 10 40 50

   * - Rank
     - Operator
     - Description
   * - 1
     - ``( )``
     - Parenthesized expression
   * - 2
     - Function calls
     - Function and function block invocation
   * - 3
     - ``-``, ``NOT``
     - Negation, boolean complement
   * - 4
     - ``**``
     - Exponentiation
   * - 5
     - ``*``, ``/``, ``MOD``
     - Multiply, divide, modulo
   * - 6
     - ``+``, ``-``
     - Add, subtract
   * - 7
     - ``<``, ``>``, ``<=``, ``>=``
     - Comparison
   * - 8
     - ``=``, ``<>``
     - Equality, inequality
   * - 9
     - ``AND``, ``&``
     - Boolean AND
   * - 10
     - ``XOR``
     - Boolean exclusive OR
   * - 11
     - ``OR``
     - Boolean OR

.. toctree::
   :maxdepth: 1
   :hidden:

   assignment
   bit-access
   if
   case
   for
   while
   repeat
   exit
   return
   arithmetic-operators
   comparison-operators
   logical-operators
   function-call
