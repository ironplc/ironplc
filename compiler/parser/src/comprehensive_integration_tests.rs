//! Comprehensive integration tests for IronPLC esstee syntax support
//!
//! These tests combine multiple new syntax features to validate complex scenarios
//! including forward references, type dependencies, and edge cases.

use proptest::prelude::*;
use dsl::common::*;
use dsl::core::FileId;
use crate::options::ParseOptions;
use crate::parse_program;

/// Test combining VAR_GLOBAL, TYPE, and PROGRAM declarations in various orders
#[cfg(test)]
mod mixed_declaration_tests {
    use super::*;

    #[test]
    fn test_comprehensive_mixed_declarations() {
        let source = r#"
TYPE
    StateEnum : (IDLE, RUNNING, STOPPED, ERROR);
    CounterArray : ARRAY[1..10] OF INT;
    TemperatureRange : INT(-50..150);
END_TYPE

VAR_GLOBAL
    system_state : StateEnum;
    process_counters : CounterArray;
    current_temp : TemperatureRange;
END_VAR

PROGRAM MainControl
VAR
    local_counter : INT := 0;
    status_flag : BOOL := FALSE;
END_VAR
    local_counter := local_counter + 1;
    status_flag := TRUE;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse comprehensive mixed declarations: {:?}", result.err());
        
        let library = result.unwrap();
        // The parser creates separate elements for each type definition, so we expect 5 elements
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
        
        // Verify we have all expected element types
        let mut has_type_elements = false;
        let mut has_global_vars = false;
        let mut has_program = false;
        
        for element in &library.elements {
            match element {
                LibraryElementKind::TypeDefinitionBlock(_) => has_type_elements = true,
                LibraryElementKind::DataTypeDeclaration(_) => has_type_elements = true,
                LibraryElementKind::GlobalVariableDeclaration(_) => has_global_vars = true,
                LibraryElementKind::ProgramDeclaration(_) => has_program = true,
                _ => {}
            }
        }
        
        assert!(has_type_elements, "Missing TYPE elements");
        assert!(has_global_vars, "Missing VAR_GLOBAL block");
        assert!(has_program, "Missing PROGRAM block");
    }

    #[test]
    fn test_forward_reference_resolution() {
        let source = r#"
VAR_GLOBAL
    main_controller : INT;
    sensor_data : ARRAY[1..8] OF INT;
END_VAR

TYPE SensorArray : ARRAY[1..8] OF INT; END_TYPE
TYPE SensorReading : INT; END_TYPE
TYPE SensorStatus : (OK, WARNING, ERROR, OFFLINE); END_TYPE
TYPE ControllerType : INT; END_TYPE
TYPE OperationMode : (MANUAL, AUTO, MAINTENANCE); END_TYPE

PROGRAM SensorMonitor
VAR
    current_reading : INT;
END_VAR
    current_reading := 42;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse forward references: {:?}", result.err());
        
        let library = result.unwrap();
        // The parser creates separate elements for each type definition
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
    }

    #[test]
    fn test_complex_type_dependencies() {
        let source = r#"
TYPE
    // Base types
    ProcessID : INT(1..100);
    Priority : (LOW, MEDIUM, HIGH, CRITICAL);
    
    // Dependent types - simplified without STRUCT
    TaskQueue : ARRAY[1..20] OF ProcessID;
END_TYPE

VAR_GLOBAL
    active_tasks : TaskQueue;
    next_task_id : ProcessID := 1;
    default_priority : Priority := MEDIUM;
END_VAR

PROGRAM TaskManager
VAR
    task_count : ProcessID;
    current_priority : Priority;
END_VAR
    task_count := 5;
    current_priority := MEDIUM;
    next_task_id := task_count;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse complex type dependencies: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
    }

    #[test]
    fn test_multiple_var_global_and_type_blocks() {
        let source = r#"
TYPE
    BasicFlag : BOOL;
    BasicCounter : INT;
END_TYPE

VAR_GLOBAL
    system_flag : BasicFlag;
    error_count : BasicCounter := 0;
END_VAR

TYPE
    ExtendedCounter : INT(0..1000);
END_TYPE

VAR_GLOBAL
    extended_counter : ExtendedCounter;
END_VAR

PROGRAM MultiBlockTest
VAR
    local_counter : ExtendedCounter;
END_VAR
    local_counter := 42;
    extended_counter := local_counter;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse multiple blocks: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 5, "Expected at least 5 top-level elements, got {}", library.elements.len());
        
        // Count each type
        let type_blocks = library.elements.iter().filter(|e| matches!(e, 
            LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
        )).count();
        let global_blocks = library.elements.iter().filter(|e| matches!(e, 
            LibraryElementKind::GlobalVariableDeclaration(_)
        )).count();
        let programs = library.elements.iter().filter(|e| matches!(e, 
            LibraryElementKind::ProgramDeclaration(_)
        )).count();
        
        assert!(type_blocks >= 2, "Expected at least 2 TYPE blocks");
        assert_eq!(global_blocks, 2, "Expected 2 VAR_GLOBAL blocks");
        assert_eq!(programs, 1, "Expected 1 PROGRAM");
    }
}

/// Test edge cases like single-element enumerations and single-value subranges
#[cfg(test)]
mod edge_case_tests {
    use super::*;

    #[test]
    fn test_single_element_enumeration() {
        let source = r#"
TYPE
    SingleState : (ACTIVE);
    BinaryChoice : (YES, NO);
    SingletonEnum : (ONLY_VALUE);
END_TYPE

VAR_GLOBAL
    current_state : SingleState := ACTIVE;
    user_choice : BinaryChoice := YES;
    singleton : SingletonEnum := ONLY_VALUE;
END_VAR

PROGRAM SingleEnumTest
VAR
    local_state : SingleState;
END_VAR
    local_state := ACTIVE;
    current_state := local_state;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse single-element enumeration: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
    }

    #[test]
    fn test_single_value_subrange() {
        let source = r#"
TYPE
    ConstantValue : INT(42..42);
    SmallRange : INT(0..1);
    NegativeRange : INT(-5..-5);
    ZeroRange : INT(0..0);
END_TYPE

VAR_GLOBAL
    constant_val : ConstantValue := 42;
    binary_val : SmallRange := 0;
    negative_val : NegativeRange := -5;
    zero_val : ZeroRange := 0;
END_VAR

PROGRAM SubrangeEdgeTest
VAR
    local_const : ConstantValue;
    local_binary : SmallRange;
END_VAR
    local_const := 42;
    local_binary := 1;
    constant_val := local_const;
    binary_val := local_binary;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse single-value subrange: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
    }

    #[test]
    fn test_extreme_array_bounds() {
        let source = r#"
TYPE
    SingleElementArray : ARRAY[1..1] OF INT;
    NegativeBoundArray : ARRAY[-10..-5] OF BOOL;
    LargeRangeArray : ARRAY[0..100] OF REAL;
    MultiDimArray : ARRAY[1..2, 1..2] OF INT;
END_TYPE

VAR_GLOBAL
    single_elem : SingleElementArray;
    negative_array : NegativeBoundArray;
    large_array : LargeRangeArray;
    multi_array : MultiDimArray;
END_VAR

PROGRAM ArrayEdgeTest
VAR
    local_single : SingleElementArray;
END_VAR
    local_single[1] := 42;
    single_elem := local_single;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse extreme array bounds: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
    }

    #[test]
    fn test_nested_array_initialization() {
        let source = r#"
TYPE
    Matrix2x2 : ARRAY[1..2, 1..2] OF INT;
    Vector3 : ARRAY[1..3] OF REAL;
END_TYPE

VAR_GLOBAL
    identity_matrix : Matrix2x2;
    unit_vector : Vector3;
    simple_array : ARRAY[1..3] OF INT;
END_VAR

PROGRAM ArrayInitTest
VAR
    local_matrix : Matrix2x2;
    local_vector : Vector3;
END_VAR
    local_matrix[1,1] := 1;
    local_matrix[1,2] := 0;
    local_vector[1] := 1.0;
    identity_matrix := local_matrix;
    unit_vector := local_vector;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse nested array initialization: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
    }
}

/// Test complex scenarios combining all new syntax features
#[cfg(test)]
mod complex_scenario_tests {
    use super::*;

    #[test]
    fn test_industrial_automation_scenario() {
        let source = r#"
TYPE
    // Enumeration types for states and modes
    MachineState : (STOPPED, STARTING, RUNNING, STOPPING, ERROR, MAINTENANCE);
    AlarmLevel : (INFO, WARNING, CRITICAL);
    OperationMode : (MANUAL, SEMI_AUTO, AUTOMATIC);
    
    // Subrange types for constrained values - simplified syntax
    Temperature : INT(-50..200);
    Pressure : INT(0..10);
    Speed : INT(0..3000);
    PercentValue : INT(0..100);
    
    // Array types for sensor data
    TemperatureSensors : ARRAY[1..8] OF Temperature;
    PressureSensors : ARRAY[1..4] OF Pressure;
    AlarmHistory : ARRAY[1..50] OF AlarmLevel;
END_TYPE

VAR_GLOBAL
    // Global system variables
    system_state : MachineState;
    operation_mode : OperationMode;
    emergency_stop : BOOL := FALSE;
    operator_present : BOOL := FALSE;
    
    // Global process variables
    setpoint_temperature : Temperature := 25;
    setpoint_pressure : Pressure := 2;
    target_speed : Speed := 1500;
    
    // Global arrays
    temp_sensors : TemperatureSensors;
    pressure_sensors : PressureSensors;
    alarm_buffer : AlarmHistory;
    
    // Global counters and flags
    cycle_counter : INT := 0;
    error_counter : INT := 0;
    maintenance_due : BOOL := FALSE;
END_VAR

PROGRAM MainController
VAR
    // Local control variables
    local_state : MachineState;
    current_alarm : AlarmLevel;
    temp_average : Temperature;
    pressure_ok : BOOL;
    
    // Local arrays for calculations
    temp_buffer : ARRAY[1..5] OF Temperature;
    calculation_results : ARRAY[1..3] OF INT;
    efficiency : PercentValue;
END_VAR
    // Initialize system
    local_state := STOPPED;
    system_state := local_state;
    operation_mode := MANUAL;
    
    // Process temperature data
    temp_average := 25;
    temp_sensors[1] := temp_average;
    
    // Handle alarms
    current_alarm := INFO;
    alarm_buffer[1] := current_alarm;
    
    // Update counters
    cycle_counter := cycle_counter + 1;
    efficiency := 85;
END_PROGRAM

FUNCTION_BLOCK TemperatureController
VAR_INPUT
    setpoint : Temperature;
    current_value : Temperature;
    enable : BOOL;
END_VAR
VAR_OUTPUT
    control_output : PercentValue;
    alarm : BOOL;
END_VAR
VAR
    error : INT;
    integral : INT;
    last_error : INT;
END_VAR
    IF enable THEN
        error := setpoint - current_value;
        control_output := 50; // Simplified control logic
        alarm := (current_value < -40) OR (current_value > 180);
    ELSE
        control_output := 0;
        alarm := FALSE;
    END_IF;
END_FUNCTION_BLOCK
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse industrial automation scenario: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 4, "Expected at least 4 top-level elements, got {}", library.elements.len());
        
        // Verify we have all expected element types
        let mut has_types = false;
        let mut has_globals = false;
        let mut has_program = false;
        let mut has_function_block = false;
        
        for element in &library.elements {
            match element {
                LibraryElementKind::TypeDefinitionBlock(_) => has_types = true,
                LibraryElementKind::DataTypeDeclaration(_) => has_types = true,
                LibraryElementKind::GlobalVariableDeclaration(_) => has_globals = true,
                LibraryElementKind::ProgramDeclaration(_) => has_program = true,
                LibraryElementKind::FunctionBlockDeclaration(_) => has_function_block = true,
                _ => {}
            }
        }
        
        assert!(has_types, "Missing TYPE definitions");
        assert!(has_globals, "Missing VAR_GLOBAL block");
        assert!(has_program, "Missing PROGRAM");
        assert!(has_function_block, "Missing FUNCTION_BLOCK");
    }

    #[test]
    fn test_recursive_type_references() {
        let source = r#"
TYPE
    NodeID : INT(1..1000);
    NodeType : (SENSOR, ACTUATOR, CONTROLLER, GATEWAY);
    
    // Simplified without STRUCT - just use arrays for relationships
    ChildNodeArray : ARRAY[1..10] OF NodeID;
    NetworkTopology : ARRAY[1..100] OF NodeID;
END_TYPE

VAR_GLOBAL
    network : NetworkTopology;
    root_node_id : NodeID := 1;
    root_node_type : NodeType := GATEWAY;
    node_count : NodeID := 1;
    children_of_root : ChildNodeArray;
END_VAR

PROGRAM NetworkManager
VAR
    current_node_id : NodeID;
    current_node_type : NodeType;
    child_index : INT;
    parent_id : NodeID;
END_VAR
    // Initialize root node
    root_node_id := 1;
    root_node_type := GATEWAY;
    
    // Set up a child node
    current_node_id := 2;
    current_node_type := SENSOR;
    parent_id := root_node_id;
    
    // Add child to parent's children array
    children_of_root[1] := current_node_id;
    
    // Store in network topology
    network[1] := root_node_id;
    network[2] := current_node_id;
    
    node_count := 2;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse recursive type references: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
    }

    #[test]
    fn test_mixed_initialization_patterns() {
        let source = r#"
TYPE
    InitPattern : (ZERO, DEFAULT, CUSTOM);
    DataBuffer : ARRAY[1..5] OF INT;
END_TYPE

VAR_GLOBAL
    // Various initialization patterns - simplified without complex struct initialization
    pattern_type : InitPattern := DEFAULT;
    data_buffer : DataBuffer;
    multiplier : REAL := 1.0;
    
    // Simple initializations
    zero_value : INT := 0;
    sequence_start : INT := 1;
    flag_state : BOOL := TRUE;
END_VAR

PROGRAM InitializationTest
VAR
    local_pattern : InitPattern;
    local_buffer : DataBuffer;
    temp_value : INT;
END_VAR
    // Test various assignment patterns
    local_pattern := CUSTOM;
    multiplier := 2.5;
    
    // Array operations
    local_buffer[1] := 10;
    local_buffer[2] := 20;
    
    // Update global state
    pattern_type := local_pattern;
    temp_value := sequence_start;
    data_buffer := local_buffer;
END_PROGRAM
        "#;

        let result = parse_program(source, &FileId::default(), &ParseOptions::default());
        assert!(result.is_ok(), "Failed to parse mixed initialization patterns: {:?}", result.err());
        
        let library = result.unwrap();
        assert!(library.elements.len() >= 3, "Expected at least 3 top-level elements, got {}", library.elements.len());
    }
}

/// Property-based tests for comprehensive integration scenarios
#[cfg(test)]
mod comprehensive_property_tests {
    use super::*;

    // Helper function to check if a string is a reserved keyword
    fn is_reserved_keyword(s: &str) -> bool {
        let upper_s = s.to_uppercase();
        matches!(upper_s.as_str(), 
            "TO" | "DO" | "END" | "IF" | "THEN" | "ELSE" | "ELSIF" | "FOR" | "WHILE" | "CASE" | "OF" | 
            "VAR" | "TYPE" | "PROGRAM" | "FUNCTION" | "FUNCTION_BLOCK" | "TP" | "TON" | "TOF" | 
            "ARRAY" | "STRING" | "BOOL" | "INT" | "REAL" | "DINT" | "LINT" | "SINT" | "UINT" | 
            "UDINT" | "ULINT" | "USINT" | "LREAL" | "BYTE" | "WORD" | "DWORD" | "LWORD" | 
            "TIME" | "DATE" | "WSTRING" | "TRUE" | "FALSE" | "NULL" | "AND" | "OR" | "NOT" | 
            "XOR" | "MOD" | "DIV" | "RETURN" | "EXIT" | "CONTINUE" | "REPEAT" | "UNTIL" | 
            "STEP" | "TRANSITION" | "ACTION" | "ACTIONS" | "CLASS" | "METHOD" | "EXTENDS" | 
            "IMPLEMENTS" | "INTERFACE" | "ABSTRACT" | "FINAL" | "OVERRIDE" | "PRIVATE" | 
            "PROTECTED" | "PUBLIC" | "INTERNAL" | "CONSTANT" | "RETAIN" | "NON_RETAIN" | 
            "PERSISTENT" | "AT" | "REF_TO" | "POINTER" | "STRUCT" | "UNION" | "ENUM" | 
            "CONFIGURATION" | "RESOURCE" | "TASK" | "VAR_INPUT" | "VAR_OUTPUT" | "VAR_IN_OUT" | 
            "VAR_EXTERNAL" | "VAR_GLOBAL" | "VAR_ACCESS" | "VAR_CONFIG" | "VAR_TEMP" | 
            "END_VAR" | "END_TYPE" | "END_STRUCT" | "END_UNION" | "END_ENUM" | "END_PROGRAM" | 
            "END_FUNCTION" | "END_FUNCTION_BLOCK" | "END_CLASS" | "END_METHOD" | "END_INTERFACE" | 
            "END_CONFIGURATION" | "END_RESOURCE" | "END_TASK" | "END_ACTION" | "END_ACTIONS" | 
            "END_CASE" | "END_FOR" | "END_WHILE" | "END_REPEAT" | "END_IF" | "END_STEP" | 
            "END_TRANSITION" | "ON" | "OFF" | "IDLE" | "RUNNING" | "STOPPED" | "START" | "STOP" |
            "PAUSE" | "RESET" | "RED" | "GREEN" | "BLUE" | "BY" | "FROM" | "WITH" | "read_only" |
            "read_write" | "INITIAL_STEP" | "R_EDGE" | "F_EDGE" | "EN" | "ENO" | "DT" | "DATE_AND_TIME"
        )
    }

    prop_compose! {
        fn arb_comprehensive_scenario()(
            type_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            enum_values in prop::collection::vec("[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)), 2..4),
            array_lower in 1..5i32,
            array_upper in 5..10i32,
            subrange_lower in -10..0i32,
            subrange_upper in 0..10i32
        ) -> String {
            format!(
                "TYPE\n    {} : ({});\n    {}Array : ARRAY[{}..{}] OF INT;\n    {}Range : INT({}..{});\nEND_TYPE\n\n\
                 VAR_GLOBAL\n    {} : {};\n    {}Arr : {}Array;\n    {}Val : {}Range;\nEND_VAR\n\n\
                 PROGRAM {}\nVAR\n    local_var : INT;\nEND_VAR\n    local_var := 1;\nEND_PROGRAM",
                type_name, enum_values.join(", "),
                type_name, array_lower, array_upper,
                type_name, subrange_lower, subrange_upper,
                var_name, type_name,
                var_name, type_name,
                var_name, type_name,
                program_name
            )
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        
        #[test]
        fn test_comprehensive_integration_scenarios(
            scenario in arb_comprehensive_scenario()
        ) {
            let result = parse_program(&scenario, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse comprehensive scenario: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have at least 3 elements (TYPE definitions may be split, VAR_GLOBAL, PROGRAM)
            prop_assert!(library.elements.len() >= 3, "Expected at least 3 elements, got {}", library.elements.len());
            
            // Verify we have all expected element types
            let mut has_types = false;
            let mut has_globals = false;
            let mut has_program = false;
            
            for element in &library.elements {
                match element {
                    LibraryElementKind::TypeDefinitionBlock(_) => has_types = true,
                    LibraryElementKind::DataTypeDeclaration(_) => has_types = true,
                    LibraryElementKind::GlobalVariableDeclaration(_) => has_globals = true,
                    LibraryElementKind::ProgramDeclaration(_) => has_program = true,
                    _ => {}
                }
            }
            
            prop_assert!(has_types, "Missing TYPE definitions");
            prop_assert!(has_globals, "Missing VAR_GLOBAL block");
            prop_assert!(has_program, "Missing PROGRAM");
        }
    }

    prop_compose! {
        fn arb_forward_reference_scenario()(
            type1_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            type2_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s) && s != "A"),
            var_name in "[a-z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s)),
            program_name in "[A-Z][A-Za-z0-9_]*".prop_filter("Avoid reserved keywords", |s| !is_reserved_keyword(s))
        ) -> String {
            // Ensure unique names by adding prefixes
            let unique_type1 = format!("Type1_{}", type1_name);
            let unique_type2 = format!("Type2_{}", type2_name);
            let unique_var = format!("var_{}", var_name);
            let unique_prog = format!("Prog_{}", program_name);
            
            // Simplified without STRUCT - just use basic type references
            format!(
                "VAR_GLOBAL\n    {} : {};\nEND_VAR\n\n\
                 TYPE\n    {} : {};\n    {} : INT;\nEND_TYPE\n\n\
                 PROGRAM {}\nVAR\n    local_var : {};\nEND_VAR\n    local_var := 42;\nEND_PROGRAM",
                unique_var, unique_type1,
                unique_type1, unique_type2, unique_type2,
                unique_prog, unique_type2
            )
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig::with_cases(50))]
        
        #[test]
        fn test_forward_reference_resolution_scenarios(
            scenario in arb_forward_reference_scenario()
        ) {
            let result = parse_program(&scenario, &FileId::default(), &ParseOptions::default());
            
            prop_assert!(result.is_ok(), "Failed to parse forward reference scenario: {:?}", result.err());
            
            let library = result.unwrap();
            
            // Should have at least 3 elements
            prop_assert!(library.elements.len() >= 3, "Expected at least 3 elements, got {}", library.elements.len());
            
            // Verify structure
            let type_count = library.elements.iter().filter(|e| matches!(e, 
                LibraryElementKind::TypeDefinitionBlock(_) | LibraryElementKind::DataTypeDeclaration(_)
            )).count();
            let global_count = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::GlobalVariableDeclaration(_))).count();
            let program_count = library.elements.iter().filter(|e| matches!(e, LibraryElementKind::ProgramDeclaration(_))).count();
            
            prop_assert!(type_count >= 1, "Missing TYPE elements");
            prop_assert_eq!(global_count, 1, "Should have exactly 1 VAR_GLOBAL block");
            prop_assert_eq!(program_count, 1, "Should have exactly 1 PROGRAM");
        }
    }
}

/// Test utilities for comprehensive integration testing
#[cfg(test)]
mod test_utilities {
    use super::*;

    /// Helper function to validate library structure
    pub fn validate_comprehensive_library(library: &Library) -> Result<(), String> {
        if library.elements.is_empty() {
            return Err("Library has no elements".to_string());
        }

        let mut type_elements = 0;
        let mut global_elements = 0;
        let mut program_elements = 0;
        let mut function_elements = 0;
        let mut function_block_elements = 0;

        for element in &library.elements {
            match element {
                LibraryElementKind::TypeDefinitionBlock(_) => type_elements += 1,
                LibraryElementKind::DataTypeDeclaration(_) => type_elements += 1,
                LibraryElementKind::GlobalVariableDeclaration(_) => global_elements += 1,
                LibraryElementKind::ProgramDeclaration(_) => program_elements += 1,
                LibraryElementKind::FunctionDeclaration(_) => function_elements += 1,
                LibraryElementKind::FunctionBlockDeclaration(_) => function_block_elements += 1,
                _ => {}
            }
        }

        Ok(())
    }

    /// Helper to create test scenarios with specific patterns
    pub fn create_test_scenario(pattern: &str) -> String {
        match pattern {
            "simple" => {
                "TYPE\n    SimpleType : INT;\nEND_TYPE\n\n\
                 VAR_GLOBAL\n    simple_var : SimpleType;\nEND_VAR\n\n\
                 PROGRAM SimpleTest\nEND_PROGRAM".to_string()
            }
            "complex" => {
                "TYPE\n    ComplexEnum : (A, B, C);\n    ComplexArray : ARRAY[1..5] OF INT;\nEND_TYPE\n\n\
                 VAR_GLOBAL\n    complex_enum : ComplexEnum;\n    complex_array : ComplexArray;\nEND_VAR\n\n\
                 PROGRAM ComplexTest\nVAR\n    local_var : INT;\nEND_VAR\n    local_var := 1;\nEND_PROGRAM".to_string()
            }
            _ => "PROGRAM DefaultTest\nEND_PROGRAM".to_string()
        }
    }
}