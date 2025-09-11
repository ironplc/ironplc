PROGRAM main_program
VAR
    counter : INT := 0;
    flag : BOOL := FALSE;
    temperature : REAL := 25.5;
END_VAR
END_PROGRAM

FUNCTION_BLOCK FB_Controller
VAR
    setpoint : REAL := 100.0;
    output : BOOL := FALSE;
    status : INT := 0;
END_VAR
END_FUNCTION_BLOCK

FUNCTION_BLOCK FB_Logger
VAR
    log_level : INT := 1;
    enabled : BOOL := TRUE;
END_VAR
END_FUNCTION_BLOCK
