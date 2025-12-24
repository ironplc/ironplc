===============
Extended Syntax
===============

IronPLC supports several extended syntax features beyond the standard IEC 61131-3 specification. These features are commonly used in industrial PLC programming and enhance the expressiveness and functionality of Structured Text programs.

External Functions
==================

External functions allow you to declare functions that are implemented outside the current compilation unit, such as in external libraries or runtime systems.

Syntax
------

External functions can be declared using two annotation formats:

.. code-block:: st

   // Curly brace annotation
   {external}
   FUNCTION ExternalMath : REAL
       VAR_INPUT
           x : REAL;
           y : REAL;
       END_VAR
   END_FUNCTION

   // At-symbol annotation  
   @EXTERNAL
   FUNCTION SystemCall : BOOL
       VAR_INPUT
           command : STRING;
       END_VAR
   END_FUNCTION

Usage
-----

External functions are called like regular functions but do not require a function body implementation:

.. code-block:: st

   PROGRAM Main
       VAR
           result : REAL;
           success : BOOL;
       END_VAR
       
       result := ExternalMath(3.14, 2.71);
       success := SystemCall('shutdown');
   END_PROGRAM

Reference Parameters
====================

Reference parameters allow functions to modify their arguments by passing the variable's address rather than its value.

Syntax
------

Parameters are marked as references using the ``{ref}`` annotation:

.. code-block:: st

   FUNCTION SwapValues
       VAR_INPUT
           {ref} a : INT;
           {ref} b : INT;
       END_VAR
       VAR
           temp : INT;
       END_VAR
       
       temp := a;
       a := b;
       b := temp;
   END_FUNCTION

Usage
-----

Reference parameters must be called with assignable variables (lvalues):

.. code-block:: st

   PROGRAM Main
       VAR
           x : INT := 10;
           y : INT := 20;
       END_VAR
       
       SwapValues(x, y);  // x is now 20, y is now 10
   END_PROGRAM

C-Style Comments
================

In addition to IEC 61131-3 standard comments ``(* ... *)``, IronPLC supports C-style line comments.

Syntax
------

.. code-block:: st

   PROGRAM Main
       VAR
           counter : INT := 0;  // Initialize counter to zero
       END_VAR
       
       // Increment the counter each cycle
       counter := counter + 1;
       
       (* Traditional IEC comment style also supported *)
   END_PROGRAM

Classes and Methods
===================

Object-oriented programming features allow you to organize code into reusable classes with methods and instance variables.

Syntax
------

.. code-block:: st

   CLASS Motor
       VAR
           speed : REAL;
           running : BOOL;
       END_VAR
       
       METHOD Start : BOOL
           running := TRUE;
           Start := TRUE;
       END_METHOD
       
       METHOD Stop
           running := FALSE;
           speed := 0.0;
       END_METHOD
       
       METHOD SetSpeed
           VAR_INPUT
               newSpeed : REAL;
           END_VAR
           
           IF running THEN
               speed := newSpeed;
           END_IF;
       END_METHOD
   END_CLASS

Usage
-----

.. code-block:: st

   PROGRAM Main
       VAR
           motor1 : Motor;
           success : BOOL;
       END_VAR
       
       success := motor1.Start();
       motor1.SetSpeed(1500.0);
       motor1.Stop();
   END_PROGRAM

Action Blocks
=============

Action blocks provide a way to organize code into named, reusable sections within programs.

Syntax
------

.. code-block:: st

   PROGRAM Main
       VAR
           step : INT := 0;
           timer : TON;
       END_VAR
       
       CASE step OF
           0: Initialize();
           1: ProcessData();
           2: Cleanup();
       END_CASE;
       
       ACTIONS
           ACTION Initialize
               // Initialization code
               step := 1;
           END_ACTION
           
           ACTION ProcessData
               // Main processing logic
               IF timer.Q THEN
                   step := 2;
               END_IF;
           END_ACTION
           
           ACTION Cleanup
               // Cleanup operations
               step := 0;
           END_ACTION
       END_ACTIONS
   END_PROGRAM

Reference Types and Pointer Operations
======================================

Reference types provide pointer-like functionality for indirect access to variables.

Syntax
------

.. code-block:: st

   TYPE
       IntRef : REF_TO INT;
   END_TYPE

   PROGRAM Main
       VAR
           value : INT := 42;
           ptr : IntRef;
           result : INT;
       END_VAR
       
       ptr := &value;        // Take address of value
       result := ptr^;       // Dereference pointer
       ptr^ := 100;          // Modify through pointer
       ptr := NULL;          // Null assignment
   END_PROGRAM

Complex Reference Operations
----------------------------

.. code-block:: st

   TYPE
       IntRefRef : REF_TO REF_TO INT;
   END_TYPE

   PROGRAM Advanced
       VAR
           value : INT := 42;
           ptr : REF_TO INT;
           ptrptr : IntRefRef;
           result : INT;
       END_VAR
       
       ptr := &value;
       ptrptr := &ptr;
       result := ptrptr^^;   // Double dereference
   END_PROGRAM

Arrays and Structs
==================

Enhanced array and struct operations with bounds checking and member access.

Array Operations
----------------

.. code-block:: st

   PROGRAM ArrayExample
       VAR
           numbers : ARRAY[1..10] OF INT;
           matrix : ARRAY[1..3, 1..3] OF REAL;
           i, j : INT;
       END_VAR
       
       // Single-dimensional array access
       numbers[5] := 42;
       
       // Multi-dimensional array access
       FOR i := 1 TO 3 DO
           FOR j := 1 TO 3 DO
               matrix[i, j] := i * j;
           END_FOR;
       END_FOR;
   END_PROGRAM

Struct Operations
-----------------

.. code-block:: st

   TYPE
       Point : STRUCT
           x : REAL;
           y : REAL;
       END_STRUCT;
       
       Line : STRUCT
           start : Point;
           end : Point;
       END_STRUCT;
   END_TYPE

   PROGRAM StructExample
       VAR
           p1 : Point;
           line1 : Line;
       END_VAR
       
       // Member access
       p1.x := 10.0;
       p1.y := 20.0;
       
       // Nested member access
       line1.start.x := 0.0;
       line1.start.y := 0.0;
       line1.end := p1;
   END_PROGRAM

Control Flow Extensions
=======================

Continue Statement
------------------

The ``CONTINUE`` statement allows skipping to the next iteration of a loop:

.. code-block:: st

   PROGRAM LoopExample
       VAR
           i : INT;
           sum : INT := 0;
       END_VAR
       
       FOR i := 1 TO 10 DO
           IF i MOD 2 = 0 THEN
               CONTINUE;  // Skip even numbers
           END_IF;
           sum := sum + i;
       END_FOR;
       
       // sum now contains 1+3+5+7+9 = 25
   END_PROGRAM

Range-Constrained Types
=======================

Range-constrained types allow you to define variables with specific value bounds for safety and optimization.

Syntax
------

.. code-block:: st

   TYPE
       Percentage : DINT(0..100);
       Temperature : REAL(-40.0..150.0);
   END_TYPE

   PROGRAM RangeExample
       VAR
           humidity : Percentage := 50;
           temp : Temperature;
       END_VAR
       
       humidity := 75;      // Valid assignment
       temp := 25.5;        // Valid assignment
       
       // The following would generate runtime errors:
       // humidity := 150;  // Out of range
       // temp := -50.0;    // Below minimum
   END_PROGRAM

Error Handling
==============

The extended syntax features include comprehensive error handling:

Parse Errors
------------

- Invalid annotation syntax generates clear error messages
- Malformed class and method declarations are detected
- Invalid reference operations are caught during parsing

Semantic Errors
---------------

- Type mismatches in reference parameters are reported
- Null pointer dereferences are detected
- Array bounds violations are checked
- Range constraint violations are validated

Runtime Errors
---------------

- Null pointer access protection
- Array index bounds checking
- Range constraint validation

Best Practices
==============

1. **External Functions**: Use external functions sparingly and ensure proper type safety
2. **Reference Parameters**: Only use when necessary for performance or when modification is required
3. **Classes**: Organize related functionality into cohesive classes
4. **Action Blocks**: Use for state machine implementations and modular code organization
5. **Reference Types**: Handle null pointers carefully and validate before dereferencing
6. **Range Types**: Use for critical safety parameters and optimization opportunities

Compatibility
=============

All extended syntax features are backward compatible with standard IEC 61131-3 code. Existing programs will continue to work without modification.