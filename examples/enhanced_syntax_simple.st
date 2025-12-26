(*
 * Enhanced Syntax Simple Example
 * 
 * This example demonstrates the enhanced IEC 61131-3 syntax features
 * supported by the IronPLC parser using proven syntax patterns.
 *)

(* Type definitions *)
TYPE
    Temperature : REAL;
    Percentage : INT(0..100);
    SystemMode : (MANUAL, AUTOMATIC, MAINTENANCE);
END_TYPE

(* Global variables *)
VAR_GLOBAL
    (* Basic variables *)
    system_enabled : BOOL;
    cycle_count : INT;
    
    (* Variables using custom types *)
    current_temp : Temperature;
    efficiency : Percentage;
    mode : SystemMode;
    
    (* Array declarations *)
    sensor_data : ARRAY[1..8] OF REAL;
    motor_speeds : ARRAY[0..3] OF INT;
    
    (* Multi-dimensional array *)
    control_matrix : ARRAY[1..3, 1..4] OF BOOL;
    
    (* Subrange variables *)
    ambient_temp : INT(-40..85);
    pressure_limit : INT(100..200);
    
    (* Enumeration variables *)
    alarm_level : (INFO, WARNING, CRITICAL);
END_VAR

(* Additional global variables *)
VAR_GLOBAL
    emergency_stop : BOOL;
    maintenance_mode : BOOL;
END_VAR

(* Main program *)
PROGRAM EnhancedDemo
VAR
    i : INT;
    local_temp : Temperature;
END_VAR

    (* Initialize system *)
    system_enabled := TRUE;
    cycle_count := 0;
    
    (* Array operations *)
    FOR i := 1 TO 8 DO
        sensor_data[i] := 20.0 + i;
    END_FOR;
    
    (* Multi-dimensional array *)
    control_matrix[1, 1] := TRUE;
    control_matrix[2, 3] := FALSE;
    
    (* Subrange operations *)
    ambient_temp := 25;
    pressure_limit := 150;
    
    (* Update counters *)
    cycle_count := cycle_count + 1;

END_PROGRAM

(* Function block example *)
FUNCTION_BLOCK SimpleController
VAR_INPUT
    setpoint : Temperature;
END_VAR
VAR_OUTPUT
    output : Percentage;
END_VAR
VAR
    error : REAL;
    mode : (AUTO, MANUAL);
END_VAR

    (* Simple control logic *)
    error := setpoint - current_temp;
    
    IF error > 5.0 THEN
        output := 100;
    ELSIF error < -5.0 THEN
        output := 0;
    ELSE
        output := 50;
    END_IF;

END_FUNCTION_BLOCK