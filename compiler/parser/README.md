# IronPLC Parser

IronPLC parser for IEC 61131-3 Structured Text language elements with enhanced syntax support.

## Overview

The IronPLC parser converts IEC 61131-3 Structured Text source code into Abstract Syntax Trees (AST) for further analysis and compilation. The parser supports the core IEC 61131-3 specification plus enhanced syntax patterns commonly found in industrial PLC programming environments.

## Supported Syntax Features

### Core IEC 61131-3 Features
- Program declarations (`PROGRAM...END_PROGRAM`)
- Function block declarations (`FUNCTION_BLOCK...END_FUNCTION_BLOCK`)
- Function declarations (`FUNCTION...END_FUNCTION`)
- Variable declarations (`VAR`, `VAR_INPUT`, `VAR_OUTPUT`, `VAR_IN_OUT`)
- Basic data types (`BOOL`, `INT`, `REAL`, `STRING`, etc.)
- Control flow statements (`IF`, `CASE`, `FOR`, `WHILE`, `REPEAT`)
- Expressions and assignments
- Comments (both `(* *)` and `//` styles)

### Enhanced Syntax Support

#### Global Variable Declarations
Support for global variable declarations accessible across all program units:

```st
VAR_GLOBAL
    global_counter : INT := 0;
    system_status : BOOL;
    temperature_array : ARRAY[1..10] OF REAL;
END_VAR
```

**Features:**
- Multiple `VAR_GLOBAL` blocks are automatically merged
- Support for initialization values
- Compatible with all data types including arrays and custom types
- Proper symbol table registration for cross-program access

#### Type Definitions
Custom data type definitions using `TYPE...END_TYPE` blocks:

```st
TYPE
    base_type : INT;
    temperature_type : REAL := 20.0;
    counter_type : base_type := 1;
END_TYPE
```

**Features:**
- Type aliases for existing types
- Default value assignments
- Forward reference resolution for type dependencies
- Nested type definitions

#### Enumeration Types
Named constant definitions for improved code readability:

```st
VAR_GLOBAL
    machine_state : (IDLE, RUNNING, STOPPED, ERROR);
    color_selection : (RED, GREEN, BLUE);
END_VAR
```

**Features:**
- Inline enumeration definitions in variable declarations
- Single-element enumerations supported
- Type checking for enumeration value assignments
- Integration with CASE statements

#### Array Types
Multi-dimensional array support with flexible bounds:

```st
VAR_GLOBAL
    simple_array : ARRAY[1..10] OF INT;
    negative_bounds : ARRAY[-5..5] OF REAL;
    multi_dim : ARRAY[0..2, 1..3, -1..1] OF BOOL;
    initialized_array : ARRAY[1..2, 1..2] OF INT := [[1,2],[3,4]];
END_VAR
```

**Features:**
- Single and multi-dimensional arrays
- Positive and negative array bounds
- Nested bracket initialization syntax
- Array bounds validation

#### Subrange Types
Constrained integer types with specified value ranges:

```st
VAR_GLOBAL
    percentage : INT(0..100) := 50;
    temperature : INT(-40..120);
    single_value : INT(42..42) := 42;
END_VAR
```

**Features:**
- Range constraint specification
- Default value validation within range
- Single-value subranges supported
- Mathematical validation of range bounds

### Error Handling and Recovery

The parser provides comprehensive error handling with:

- **Location-aware error reporting**: Precise error locations with line and column information
- **Multiple error collection**: Reports multiple syntax errors in a single compilation pass
- **Graceful error recovery**: Continues parsing after errors to find additional issues
- **Specific error messages**: Detailed error descriptions for each syntax construct
- **Suggestion engine**: Provides correction suggestions for common syntax mistakes

### JSON AST Export

The parser generates JSON-serializable AST structures that include:

- Complete syntax tree representation
- Source location information for all nodes
- Type information and symbol table data
- Structural relationships between declarations
- Backward compatibility with existing JSON schema

## Usage Examples

### Basic Program with Enhanced Syntax

```st
TYPE
    MotorState : (STOPPED, STARTING, RUNNING, STOPPING);
    TemperatureRange : INT(0..100);
END_TYPE

VAR_GLOBAL
    system_motors : ARRAY[1..4] OF MotorState;
    ambient_temp : TemperatureRange := 25;
    sensor_readings : ARRAY[1..8] OF REAL;
END_VAR

PROGRAM MainControl
VAR
    local_counter : INT;
END_VAR
    // Program logic using global variables and custom types
    system_motors[1] := RUNNING;
    IF ambient_temp > 80 THEN
        system_motors[1] := STOPPING;
    END_IF;
END_PROGRAM
```

### Mixed Declaration Order

The parser supports flexible declaration ordering:

```st
// Types can be declared before or after global variables
VAR_GLOBAL
    status : SystemStatus := IDLE;  // Forward reference
END_VAR

TYPE
    SystemStatus : (IDLE, ACTIVE, ERROR);
END_TYPE

PROGRAM Controller
    // Program implementation
END_PROGRAM
```

## Integration with IronPLC

The enhanced parser integrates seamlessly with the IronPLC compilation pipeline:

1. **Lexical Analysis**: Tokenizes source code including new syntax elements
2. **Syntax Analysis**: Parses tokens into AST using PEG grammar rules
3. **Symbol Table Construction**: Registers global variables and type definitions
4. **Forward Reference Resolution**: Resolves type dependencies in multiple passes
5. **AST Export**: Generates JSON representation for downstream tools

## Backward Compatibility

All enhancements maintain full backward compatibility with existing IronPLC functionality:

- Existing programs compile without modification
- Original JSON schema format preserved
- No performance degradation for existing code
- All existing test suites continue to pass

## Testing and Validation

The parser includes comprehensive test coverage:

- **Unit Tests**: Individual parser rules and AST node creation
- **Property-Based Tests**: Randomized testing across syntax variations
- **Integration Tests**: Complete file parsing with mixed syntax features
- **Regression Tests**: Ensures backward compatibility preservation
- **esstee Compatibility Tests**: Validates against real-world PLC code patterns

See [IronPLC](https://github.com/ironplc/ironplc) for more information.
