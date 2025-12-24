========================
Enhanced Syntax Support
========================

IronPLC's enhanced syntax support extends the compiler to handle additional IEC 61131-3 Structured Text patterns commonly found in industrial automation code. These enhancements improve compatibility with real-world PLC programs while maintaining full backward compatibility.

.. note::
   Enhanced syntax support is currently in development. Some features may require additional parser integration to be fully functional.

Overview
========

The enhanced syntax support adds the following major features:

* **STRUCT Types** - User-defined data structures for organizing related variables
* **ARRAY Types** - Multi-dimensional arrays with bounds checking
* **STRING(n) Types** - String types with specified maximum lengths
* **Timer Function Blocks** - Built-in timer support (TON, TOF, TP)
* **CASE Statements** - Multi-way conditional statements with labeled cases
* **Time Literals** - Time constants in T#5S, T#100MS format
* **Robust Comment Handling** - Enhanced parsing of decorative comment blocks
* **Comprehensive Error Reporting** - Detailed error messages with suggestions

STRUCT Types
============

STRUCT types allow you to group related variables into user-defined data structures, improving code organization and maintainability.

Basic STRUCT Declaration
------------------------

.. code-block:: st

   TYPE
       Point : STRUCT
           x : REAL;
           y : REAL;
       END_STRUCT;
       
       Motor : STRUCT
           speed : REAL;
           running : BOOL;
           current : REAL;
           temperature : REAL;
       END_STRUCT;
   END_TYPE

Variable Declaration and Usage
------------------------------

.. code-block:: st

   PROGRAM MotorControl
       VAR
           position : Point;
           motor1 : Motor;
           motor2 : Motor;
       END_VAR
       
       // Member access using dot notation
       position.x := 10.5;
       position.y := 20.3;
       
       motor1.speed := 1500.0;
       motor1.running := TRUE;
       
       // Structure assignment
       motor2 := motor1;
   END_PROGRAM

Nested STRUCT Types
-------------------

.. code-block:: st

   TYPE
       Coordinate : STRUCT
           position : Point;
           velocity : Point;
           acceleration : Point;
       END_STRUCT;
       
       Robot : STRUCT
           arm1 : Coordinate;
           arm2 : Coordinate;
           base : Point;
           status : INT;
       END_STRUCT;
   END_TYPE

   PROGRAM RobotControl
       VAR
           robot : Robot;
       END_VAR
       
       // Multi-level member access
       robot.arm1.position.x := 100.0;
       robot.arm1.velocity.y := 5.0;
       robot.base.x := 0.0;
       robot.base.y := 0.0;
   END_PROGRAM

ARRAY Types
===========

Enhanced ARRAY support provides multi-dimensional arrays with comprehensive bounds checking and type validation.

Single-Dimensional Arrays
-------------------------

.. code-block:: st

   TYPE
       IntArray : ARRAY[1..10] OF INT;
       RealArray : ARRAY[0..99] OF REAL;
       BoolArray : ARRAY[-5..5] OF BOOL;
   END_TYPE

   PROGRAM ArrayExample
       VAR
           numbers : IntArray;
           values : RealArray;
           flags : BoolArray;
           i : INT;
       END_VAR
       
       // Array element access
       numbers[1] := 42;
       numbers[10] := 100;
       
       // Array initialization loop
       FOR i := 1 TO 10 DO
           numbers[i] := i * i;
       END_FOR;
   END_PROGRAM

Multi-Dimensional Arrays
------------------------

.. code-block:: st

   TYPE
       Matrix : ARRAY[1..3, 1..3] OF REAL;
       DataCube : ARRAY[1..10, 1..5, 1..20] OF INT;
   END_TYPE

   PROGRAM MatrixOperations
       VAR
           transform : Matrix;
           data : DataCube;
           i, j, k : INT;
       END_VAR
       
       // Multi-dimensional access
       transform[1, 1] := 1.0;
       transform[2, 2] := 1.0;
       transform[3, 3] := 1.0;
       
       // Three-dimensional array access
       FOR i := 1 TO 10 DO
           FOR j := 1 TO 5 DO
               FOR k := 1 TO 20 DO
                   data[i, j, k] := i + j + k;
               END_FOR;
           END_FOR;
       END_FOR;
   END_PROGRAM

Arrays of STRUCT Types
----------------------

.. code-block:: st

   TYPE
       SensorData : STRUCT
           value : REAL;
           timestamp : TIME;
           quality : INT;
       END_STRUCT;
       
       SensorArray : ARRAY[1..16] OF SensorData;
   END_TYPE

   PROGRAM DataAcquisition
       VAR
           sensors : SensorArray;
           i : INT;
       END_VAR
       
       // Access struct members in arrays
       FOR i := 1 TO 16 DO
           sensors[i].value := 0.0;
           sensors[i].quality := 192; // Good quality
       END_FOR;
       
       // Individual sensor access
       sensors[5].value := 23.7;
       sensors[5].timestamp := T#0S;
   END_PROGRAM

STRING(n) Types
===============

STRING(n) types provide string variables with specified maximum lengths for memory control and validation.

Basic STRING(n) Declaration
---------------------------

.. code-block:: st

   TYPE
       ShortString : STRING(20);
       MediumString : STRING(80);
       LongString : STRING(255);
   END_TYPE

   PROGRAM StringHandling
       VAR
           name : ShortString;
           description : MediumString;
           message : LongString;
       END_VAR
       
       // String assignments with length validation
       name := 'Motor1';              // Valid: 6 chars <= 20
       description := 'Main drive motor for conveyor system';
       message := 'System initialized successfully';
   END_PROGRAM

String Length Validation
-----------------------

.. code-block:: st

   PROGRAM StringValidation
       VAR
           shortText : STRING(10);
           longText : STRING(100);
           result : BOOL;
       END_VAR
       
       // Valid assignments
       shortText := 'Hello';          // 5 chars <= 10: OK
       longText := shortText;         // 5 chars <= 100: OK
       
       // Length compatibility checking
       // shortText := longText;      // Would generate warning if longText > 10 chars
   END_PROGRAM

Timer Function Blocks
=====================

Enhanced timer support provides built-in timer function blocks with comprehensive type checking and time literal support.

Timer Types
-----------

.. code-block:: st

   PROGRAM TimerExample
       VAR
           startTimer : TON;          // Timer On Delay
           stopTimer : TOF;           // Timer Off Delay  
           pulseTimer : TP;           // Pulse Timer
           
           startButton : BOOL;
           stopButton : BOOL;
           motorRunning : BOOL;
       END_VAR
       
       // TON usage - delays turning ON
       startTimer(IN := startButton, PT := T#2S);
       
       // TOF usage - delays turning OFF
       stopTimer(IN := NOT stopButton, PT := T#500MS);
       
       // TP usage - generates pulse
       pulseTimer(IN := startButton, PT := T#100MS);
       
       // Timer outputs
       motorRunning := startTimer.Q AND stopTimer.Q;
   END_PROGRAM

Time Literals
-------------

.. code-block:: st

   PROGRAM TimeLiterals
       VAR
           fastTimer : TON;
           slowTimer : TON;
           preciseTimer : TON;
       END_VAR
       
       // Various time literal formats
       fastTimer(IN := TRUE, PT := T#50MS);      // 50 milliseconds
       slowTimer(IN := TRUE, PT := T#5S);        // 5 seconds
       preciseTimer(IN := TRUE, PT := T#1M30S);  // 1 minute 30 seconds
       
       // Time arithmetic
       IF fastTimer.ET >= T#25MS THEN
           // Half the preset time has elapsed
       END_IF;
   END_PROGRAM

Complex Timer Applications
-------------------------

.. code-block:: st

   TYPE
       StepTimer : STRUCT
           timer : TON;
           preset : TIME;
           active : BOOL;
       END_STRUCT;
   END_TYPE

   PROGRAM SequenceControl
       VAR
           step : INT := 0;
           stepTimers : ARRAY[1..10] OF StepTimer;
           i : INT;
       END_VAR
       
       // Initialize step timers
       stepTimers[1].preset := T#2S;
       stepTimers[2].preset := T#5S;
       stepTimers[3].preset := T#1S;
       
       // Sequence logic
       CASE step OF
           0: // Initialize
               stepTimers[1].active := TRUE;
               step := 1;
               
           1: // First step
               stepTimers[1].timer(IN := stepTimers[1].active, 
                                  PT := stepTimers[1].preset);
               IF stepTimers[1].timer.Q THEN
                   stepTimers[1].active := FALSE;
                   stepTimers[2].active := TRUE;
                   step := 2;
               END_IF;
               
           2: // Second step
               stepTimers[2].timer(IN := stepTimers[2].active,
                                  PT := stepTimers[2].preset);
               IF stepTimers[2].timer.Q THEN
                   step := 0; // Restart sequence
               END_IF;
       END_CASE;
   END_PROGRAM

CASE Statements
===============

CASE statements provide clear multi-way conditional logic for state machines and control algorithms.

Basic CASE Statement
-------------------

.. code-block:: st

   PROGRAM StateMachine
       VAR
           state : INT := 0;
           counter : INT;
       END_VAR
       
       CASE state OF
           0: // Idle state
               counter := 0;
               state := 1;
               
           1: // Running state
               counter := counter + 1;
               IF counter >= 100 THEN
                   state := 2;
               END_IF;
               
           2: // Complete state
               counter := 0;
               state := 0;
       END_CASE;
   END_PROGRAM

Multiple Case Labels
--------------------

.. code-block:: st

   TYPE
       AlarmLevel : (NONE, LOW, MEDIUM, HIGH, CRITICAL);
   END_TYPE

   PROGRAM AlarmHandler
       VAR
           currentAlarm : AlarmLevel;
           response : STRING(50);
       END_VAR
       
       CASE currentAlarm OF
           NONE:
               response := 'System normal';
               
           LOW, MEDIUM:  // Multiple labels for same action
               response := 'Minor alarm - monitor';
               
           HIGH:
               response := 'Major alarm - investigate';
               
           CRITICAL:
               response := 'Critical alarm - immediate action';
               
           ELSE  // Default case
               response := 'Unknown alarm level';
       END_CASE;
   END_PROGRAM

Nested CASE Statements
----------------------

.. code-block:: st

   TYPE
       SystemMode : (MANUAL, SEMI_AUTO, AUTO);
       SystemState : (STOPPED, STARTING, RUNNING, STOPPING, ERROR);
   END_TYPE

   PROGRAM SystemControl
       VAR
           mode : SystemMode;
           state : SystemState;
           action : STRING(30);
       END_VAR
       
       CASE mode OF
           MANUAL:
               CASE state OF
                   STOPPED: action := 'Manual ready';
                   RUNNING: action := 'Manual operation';
                   ELSE action := 'Manual mode error';
               END_CASE;
               
           SEMI_AUTO:
               CASE state OF
                   STOPPED: action := 'Semi-auto ready';
                   STARTING: action := 'Semi-auto starting';
                   RUNNING: action := 'Semi-auto running';
                   STOPPING: action := 'Semi-auto stopping';
                   ELSE action := 'Semi-auto error';
               END_CASE;
               
           AUTO:
               CASE state OF
                   STOPPED: action := 'Auto ready';
                   STARTING: action := 'Auto sequence start';
                   RUNNING: action := 'Auto sequence active';
                   STOPPING: action := 'Auto sequence stop';
                   ELSE action := 'Auto sequence error';
               END_CASE;
       END_CASE;
   END_PROGRAM

Enhanced Comment Handling
=========================

The enhanced comment parser handles decorative comment blocks commonly used in industrial PLC code for professional documentation.

Decorative Comment Blocks
-------------------------

.. code-block:: st

   (*
   ******************************************************************************
   * MOTOR CONTROL FUNCTION BLOCK
   * 
   * Description: Controls a three-phase induction motor with safety interlocks
   * Author: Plant Engineering Team
   * Date: 2024-12-22
   * Version: 2.1
   ******************************************************************************
   *)
   
   FUNCTION_BLOCK MotorControl
       VAR_INPUT
           Start : BOOL;        (* Start command from HMI *)
           Stop : BOOL;         (* Stop command - emergency or normal *)
           Reset : BOOL;        (* Fault reset command *)
       END_VAR
       
       (*
       ================================================================================
       SAFETY INTERLOCKS
       ================================================================================
       *)
       VAR
           SafetyOK : BOOL;     (* Combined safety status *)
           ThermalOK : BOOL;    (* Motor thermal protection *)
           GuardClosed : BOOL;  (* Safety guard position *)
       END_VAR
   END_FUNCTION_BLOCK

Multi-Line Comments with Content
--------------------------------

.. code-block:: st

   (*
   Process Step Sequence:
   **********************
   1. Initialize all outputs to safe state
   2. Check safety interlocks and prerequisites  
   3. Start main process timer
   4. Execute process steps in sequence
   5. Monitor for faults and alarms
   6. Complete sequence and return to idle
   
   Notes:
   ******
   - Each step has individual timeout monitoring
   - Emergency stop available at any time
   - Process data logged for quality tracking
   *)
   
   PROGRAM ProcessSequencer
       // Implementation follows...
   END_PROGRAM

Error Reporting and Diagnostics
===============================

The enhanced error reporting system provides detailed diagnostics with exact positioning and helpful suggestions.

Syntax Error Reporting
----------------------

When syntax errors occur, the compiler provides precise location information:

.. code-block:: text

   Error at line 15, column 23:
   Expected 'END_STRUCT' but found 'END_TYPE'
   
   Suggestion: STRUCT declarations must be terminated with 'END_STRUCT'
   
   Context:
   13 |     TYPE
   14 |         Point : STRUCT
   15 |             x : REAL;
   16 |             y : REAL;
   17 |         END_TYPE  <-- Error here
   18 |     END_TYPE

Type Mismatch Diagnostics
-------------------------

.. code-block:: text

   Error at line 42, column 15:
   Type mismatch in assignment
   
   Expected: REAL
   Found: STRING(20)
   
   Cannot assign string value to numeric variable
   
   Context:
   41 |     VAR
   42 |         speed : REAL := 'fast';  <-- Error here
   43 |     END_VAR

Unsupported Feature Guidance
----------------------------

.. code-block:: text

   Warning at line 28, column 8:
   Feature not yet supported: ENUM type declarations
   
   Workaround: Use INT constants instead
   
   Example:
   Instead of:
       TYPE Color : (RED, GREEN, BLUE); END_TYPE
   
   Use:
       VAR CONSTANT
           RED : INT := 0;
           GREEN : INT := 1;
           BLUE : INT := 2;
       END_VAR

Best Practices
==============

STRUCT Design Guidelines
-----------------------

1. **Logical Grouping**: Group related variables that represent a single concept
2. **Consistent Naming**: Use clear, descriptive names for both structs and members
3. **Size Considerations**: Be mindful of memory usage with large structures
4. **Nesting Depth**: Limit nesting to 3-4 levels for maintainability

.. code-block:: st

   // Good: Logical grouping
   TYPE
       MotorStatus : STRUCT
           running : BOOL;
           speed : REAL;
           current : REAL;
           temperature : REAL;
           faultCode : INT;
       END_STRUCT;
   END_TYPE
   
   // Avoid: Unrelated variables grouped together
   TYPE
       MixedData : STRUCT
           motorSpeed : REAL;
           userName : STRING(20);
           alarmCount : INT;
           systemTime : TIME;
       END_STRUCT;
   END_TYPE

ARRAY Usage Guidelines
---------------------

1. **Bounds Checking**: Always validate array indices before access
2. **Initialization**: Initialize arrays to known values
3. **Size Limits**: Consider memory constraints for large arrays
4. **Multi-dimensional**: Use sparingly and document dimensions clearly

.. code-block:: st

   PROGRAM ArrayBestPractices
       VAR
           sensorData : ARRAY[1..16] OF REAL;
           validIndex : BOOL;
           i : INT;
       END_VAR
       
       // Good: Bounds checking
       IF (i >= 1) AND (i <= 16) THEN
           sensorData[i] := ReadSensor(i);
       END_IF;
       
       // Good: Array initialization
       FOR i := 1 TO 16 DO
           sensorData[i] := 0.0;
       END_FOR;
   END_PROGRAM

STRING(n) Guidelines
-------------------

1. **Size Planning**: Choose appropriate string lengths for your data
2. **Validation**: Check string lengths before assignments when necessary
3. **Concatenation**: Be aware of length limits when building strings
4. **Localization**: Consider different language string lengths

.. code-block:: st

   TYPE
       // Good: Appropriate sizing
       PartNumber : STRING(20);      // Typical part numbers
       Description : STRING(80);     // Brief descriptions
       ErrorMessage : STRING(255);   // Detailed error text
       
       // Avoid: Excessive sizing
       SmallText : STRING(1000);     // Wasteful for short text
   END_TYPE

Timer Best Practices
-------------------

1. **Consistent Timing**: Use consistent time units within related timers
2. **Preset Management**: Consider making timer presets configurable
3. **Reset Logic**: Ensure timers are properly reset when needed
4. **Documentation**: Document timer purposes and preset values

.. code-block:: st

   PROGRAM TimerBestPractices
       VAR
           // Good: Descriptive names and consistent units
           startupDelay : TON;        // T#2S
           runTimeout : TON;          // T#30S
           shutdownDelay : TON;       // T#5S
           
           // Configuration variables
           STARTUP_TIME : TIME := T#2S;
           RUN_TIMEOUT : TIME := T#30S;
           SHUTDOWN_TIME : TIME := T#5S;
       END_VAR
       
       // Good: Use configuration variables
       startupDelay(IN := startCondition, PT := STARTUP_TIME);
   END_PROGRAM

Performance Considerations
=========================

Memory Usage
-----------

Enhanced syntax features are designed for minimal memory overhead:

* **STRUCT types**: Only allocate memory for declared instances
* **ARRAY types**: Memory allocated based on declared bounds
* **STRING(n) types**: Fixed memory allocation based on maximum length
* **Timer instances**: Minimal overhead per timer instance

Compilation Performance
----------------------

* **Type checking**: Optimized lookup tables for fast type resolution
* **Error reporting**: Efficient error collection without performance impact
* **Parser integration**: Incremental parsing for large files
* **Symbol tables**: Hash-based lookups for O(1) symbol resolution

Runtime Performance
------------------

* **Member access**: Direct offset calculation for STRUCT members
* **Array indexing**: Bounds checking with minimal overhead
* **String operations**: Length validation during compilation
* **Timer operations**: Native timer implementation efficiency

Optimization Guidelines
======================

Code Organization
----------------

1. **Type Definitions**: Group related type definitions together
2. **Variable Declarations**: Organize variables by usage patterns
3. **Function Blocks**: Use STRUCT parameters for complex data passing
4. **Modular Design**: Break large programs into smaller, focused units

.. code-block:: st

   // Good: Organized type definitions
   TYPE
       // Basic data types
       Position : STRUCT
           x : REAL;
           y : REAL;
           z : REAL;
       END_STRUCT;
       
       // Process data types
       ProcessData : STRUCT
           setpoint : REAL;
           processValue : REAL;
           output : REAL;
           mode : INT;
       END_STRUCT;
       
       // Array types
       PositionArray : ARRAY[1..10] OF Position;
       ProcessArray : ARRAY[1..5] OF ProcessData;
   END_TYPE

Memory Optimization
------------------

1. **STRUCT Packing**: Order members by size (largest first) for optimal packing
2. **Array Sizing**: Use appropriate bounds to minimize memory waste
3. **String Lengths**: Choose realistic maximum lengths
4. **Local Variables**: Prefer local variables over global when possible

.. code-block:: st

   TYPE
       // Good: Optimal member ordering (largest to smallest)
       OptimizedStruct : STRUCT
           timestamp : TIME;      // 8 bytes
           value : REAL;          // 4 bytes
           quality : INT;         // 2 bytes
           valid : BOOL;          // 1 byte
       END_STRUCT;
       
       // Less optimal: Mixed sizes
       UnoptimizedStruct : STRUCT
           valid : BOOL;          // 1 byte + 3 padding
           timestamp : TIME;      // 8 bytes
           quality : INT;         // 2 bytes + 2 padding
           value : REAL;          // 4 bytes
       END_STRUCT;
   END_TYPE

Compatibility and Migration
===========================

Backward Compatibility
---------------------

All enhanced syntax features are fully backward compatible:

* Existing IronPLC programs continue to work unchanged
* No performance impact when enhanced features are not used
* Gradual adoption possible - mix old and new syntax as needed
* All existing test suites pass without modification

Migration Strategy
-----------------

1. **Assessment**: Identify code that would benefit from enhanced syntax
2. **Gradual Adoption**: Migrate one feature at a time
3. **Testing**: Validate each migration step thoroughly
4. **Documentation**: Update code documentation to reflect new syntax

Future Enhancements
==================

Planned additions to enhanced syntax support:

* **ENUM Types**: Enumerated type support with named constants
* **UNION Types**: Union type support for memory-efficient variants
* **INTERFACE Types**: Interface definitions for object-oriented programming
* **Generic Types**: Template-like generic type support
* **Advanced Timers**: Additional timer types (TONR, CTU, CTD)

For the latest information on enhanced syntax support development, see the IronPLC project documentation and release notes.