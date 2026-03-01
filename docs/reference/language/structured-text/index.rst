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
   * - **Support**
     - Partial

Statements
----------

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Statement
     - Description
     - Status
   * - :doc:`assignment`
     - Assign a value to a variable
     - Supported
   * - :doc:`if`
     - Conditional branching
     - Supported
   * - :doc:`case`
     - Multi-way selection by integer value
     - Supported
   * - :doc:`for`
     - Counted loop
     - Supported
   * - :doc:`while`
     - Pre-tested loop
     - Supported
   * - :doc:`repeat`
     - Post-tested loop
     - Supported
   * - :doc:`exit`
     - Break from innermost loop
     - Supported
   * - :doc:`return`
     - Early exit from POU
     - Supported

Operators
---------

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Category
     - Description
     - Status
   * - :doc:`arithmetic-operators`
     - Addition, subtraction, multiplication, division, modulo, power
     - Supported
   * - :doc:`comparison-operators`
     - Equality, inequality, less than, greater than
     - Supported
   * - :doc:`logical-operators`
     - AND, OR, XOR, NOT
     - Supported

Function Calls
--------------

.. list-table::
   :header-rows: 1
   :widths: 25 45 30

   * - Topic
     - Description
     - Status
   * - :doc:`function-call`
     - Calling functions and function block instances
     - Partial

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
