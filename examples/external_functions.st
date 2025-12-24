(*
    Example demonstrating external function declarations and usage.
    This shows both annotation styles supported by IronPLC.
*)

// External function using curly brace annotation
{external}
FUNCTION MathLibSin : REAL
    VAR_INPUT
        angle : REAL;
    END_VAR
END_FUNCTION

// External function using at-symbol annotation
@EXTERNAL
FUNCTION SystemGetTime : DWORD
END_FUNCTION

// External function with multiple parameters
{external}
FUNCTION LogMessage : BOOL
    VAR_INPUT
        level : INT;
        message : STRING;
    END_VAR
END_FUNCTION

PROGRAM ExternalFunctionDemo
    VAR
        angle : REAL := 1.57;  // π/2 radians
        sine_value : REAL;
        current_time : DWORD;
        log_success : BOOL;
    END_VAR
    
    // Call external math function
    sine_value := MathLibSin(angle);
    
    // Call external system function
    current_time := SystemGetTime();
    
    // Call external logging function
    log_success := LogMessage(1, 'System initialized');
END_PROGRAM