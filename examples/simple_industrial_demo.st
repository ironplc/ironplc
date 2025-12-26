TYPE
    SystemMode : (MANUAL, AUTO, MAINTENANCE);
    SafetyLevel : (SAFE, CAUTION, DANGER);
END_TYPE

VAR_GLOBAL
    system_running : BOOL;
    current_mode : SystemMode;
    safety_status : SafetyLevel;
    
    sensor_data : ARRAY[1..8] OF REAL;
    motor_speeds : ARRAY[1..4] OF INT;
    
    cycle_count : INT;
    error_count : INT;
END_VAR

PROGRAM IndustrialControl
VAR
    i : INT;
    temp_value : REAL;
END_VAR

    (* Initialize system *)
    system_running := TRUE;
    cycle_count := 0;
    
    (* Process sensor data *)
    FOR i := 1 TO 8 DO
        sensor_data[i] := 20.0 + i;
    END_FOR;
    
    (* Set motor speeds *)
    FOR i := 1 TO 4 DO
        motor_speeds[i] := 1000 + (i * 100);
    END_FOR;
    
    (* Update counters *)
    cycle_count := cycle_count + 1;

END_PROGRAM

FUNCTION_BLOCK MotorController
VAR_INPUT
    enable : BOOL;
    speed_setpoint : INT;
END_VAR
VAR_OUTPUT
    motor_running : BOOL;
    actual_speed : INT;
END_VAR
VAR
    local_mode : (OFF, STARTING, RUNNING);
END_VAR

    IF enable THEN
        motor_running := TRUE;
        actual_speed := speed_setpoint;
    ELSE
        motor_running := FALSE;
        actual_speed := 0;
    END_IF;

END_FUNCTION_BLOCK