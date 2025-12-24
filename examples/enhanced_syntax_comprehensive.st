(* 
 * Enhanced Syntax Comprehensive Example
 * 
 * This example demonstrates all the enhanced IEC 61131-3 syntax features
 * supported by the IronPLC parser, including global variables, type definitions,
 * enumerations, arrays, and subrange types.
 *)

(* Type definitions with aliases and forward references *)
TYPE
    (* Basic type aliases *)
    Temperature : REAL := 20.0;
    Pressure : REAL := 1013.25;
    
    (* Subrange types with constraints *)
    Percentage : INT(0..100);
    MotorSpeed : INT(0..3600);  (* RPM *)
    
    (* Enumeration types *)
    SystemMode : (MANUAL, AUTOMATIC, MAINTENANCE, EMERGENCY);
    AlarmLevel : (INFO, WARNING, CRITICAL);
    
    (* Forward reference - ProcessState used before definition *)
    CurrentState : ProcessState;
    
    (* Process state enumeration *)
    ProcessState : (IDLE, INITIALIZING, RUNNING, STOPPING, ERROR);
END_TYPE

(* Global variables accessible across all program units *)
VAR_GLOBAL
    (* Basic global variables with initialization *)
    system_enabled : BOOL := FALSE;
    cycle_count : INT := 0;
    
    (* Global variables using custom types *)
    current_mode : SystemMode;
    process_state : ProcessState;
    
    (* Array declarations with various bounds *)
    sensor_temperatures : ARRAY[1..8] OF Temperature;
    motor_speeds : ARRAY[0..3] OF MotorSpeed;
    
    (* Multi-dimensional arrays *)
    control_matrix : ARRAY[1..3, 1..4] OF BOOL;
    calibration_data : ARRAY[-2..2, 0..9] OF REAL;
    
    (* Arrays with initialization *)
    default_setpoints : ARRAY[1..4] OF REAL := [25.0, 30.0, 35.0, 40.0];
    
    (* Subrange variables with constraints *)
    system_efficiency : Percentage := 85;
    ambient_temperature : INT(-40..85) := 22;
    
    (* Single-value subrange (constant) *)
    max_pressure_limit : INT(150..150) := 150;
    
    (* Enumeration variables *)
    current_alarm : AlarmLevel;
    
    (* Complex nested array initialization *)
    lookup_table : ARRAY[1..2, 1..3] OF INT := [[10, 20, 30], [40, 50, 60]];
END_VAR

(* Additional global variables in separate block - will be merged *)
VAR_GLOBAL
    (* System status flags *)
    emergency_stop : BOOL := FALSE;
    maintenance_mode : BOOL := FALSE;
    
    (* Performance counters *)
    total_runtime : INT := 0;
    error_count : INT := 0;
END_VAR

(* Main control program demonstrating usage of enhanced syntax *)
PROGRAM EnhancedSyntaxDemo
VAR
    (* Local variables *)
    local_counter : INT;
    temp_reading : Temperature;
    i, j : INT;
END_VAR

    (* Initialize system *)
    IF NOT system_enabled THEN
        process_state := INITIALIZING;
        current_mode := AUTOMATIC;
        system_enabled := TRUE;
    END_IF;
    
    (* Process sensor data using global arrays *)
    FOR i := 1 TO 8 DO
        (* Simulate temperature reading *)
        sensor_temperatures[i] := 20.0 + (i * 2.5);
    END_FOR;
    
    (* Control logic using enumerations *)
    CASE current_mode OF
        MANUAL:
            (* Manual operation *)
            process_state := RUNNING;
            
        AUTOMATIC:
            (* Automatic operation based on conditions *)
            IF system_efficiency > 90 THEN
                process_state := RUNNING;
                current_alarm := INFO;
            ELSIF system_efficiency > 70 THEN
                process_state := RUNNING;
                current_alarm := WARNING;
            ELSE
                process_state := ERROR;
                current_alarm := CRITICAL;
            END_IF;
            
        MAINTENANCE:
            (* Maintenance mode *)
            process_state := IDLE;
            maintenance_mode := TRUE;
            
        EMERGENCY:
            (* Emergency shutdown *)
            process_state := STOPPING;
            emergency_stop := TRUE;
    END_CASE;
    
    (* Multi-dimensional array operations *)
    FOR i := 1 TO 3 DO
        FOR j := 1 TO 4 DO
            control_matrix[i, j] := (i + j) MOD 2 = 0;
        END_FOR;
    END_FOR;
    
    (* Subrange validation example *)
    IF ambient_temperature > 50 THEN
        (* Temperature too high *)
        current_alarm := CRITICAL;
        process_state := ERROR;
    END_IF;
    
    (* Update performance counters *)
    cycle_count := cycle_count + 1;
    total_runtime := total_runtime + 1;
    
    (* Array lookup operations *)
    temp_reading := sensor_temperatures[1];
    
    (* Use lookup table *)
    local_counter := lookup_table[1, 2];  (* Gets value 20 *)

END_PROGRAM

(* Function block demonstrating enhanced syntax in FB context *)
FUNCTION_BLOCK TemperatureController
VAR_INPUT
    setpoint : Temperature;
    process_value : Temperature;
END_VAR
VAR_OUTPUT
    control_output : Percentage;
END_VAR
VAR
    error : REAL;
    control_mode : (PID, ON_OFF, MANUAL);
END_VAR

    (* Calculate control error *)
    error := setpoint - process_value;
    
    (* Simple control logic *)
    CASE control_mode OF
        PID:
            (* PID control (simplified) *)
            control_output := 50 + INT(error * 2.0);
            
        ON_OFF:
            (* On/off control *)
            IF error > 1.0 THEN
                control_output := 100;
            ELSIF error < -1.0 THEN
                control_output := 0;
            END_IF;
            
        MANUAL:
            (* Manual control - use global efficiency as output *)
            control_output := system_efficiency;
    END_CASE;
    
    (* Clamp output to valid percentage range *)
    IF control_output > 100 THEN
        control_output := 100;
    ELSIF control_output < 0 THEN
        control_output := 0;
    END_IF;

END_FUNCTION_BLOCK