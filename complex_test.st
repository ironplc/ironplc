PROGRAM complex_test
VAR
    counter : INT := 0;
    flag : BOOL := FALSE;
    temperature : REAL := 25.5;
    status : STRING[20] := "OK";
    data : ARRAY[1..10] OF INT;
    config : STRUCT
        enabled : BOOL;
        timeout : TIME;
    END_STRUCT;
END_VAR

FUNCTION_BLOCK FB_Test
VAR
    local_var : DINT := 100;
    temp_flag : BOOL;
END_VAR
END_FUNCTION_BLOCK

END_PROGRAM
