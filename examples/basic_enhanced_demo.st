TYPE
    base_type : INT;
    temp_type : REAL;
END_TYPE

VAR_GLOBAL
    counter : INT;
    temperature : temp_type;
    sensors : ARRAY[1..4] OF REAL;
    status : (IDLE, RUNNING, STOPPED);
END_VAR

PROGRAM BasicDemo
VAR
    i : INT;
END_VAR

    counter := 0;
    temperature := 25.0;
    
    FOR i := 1 TO 4 DO
        sensors[i] := 20.0 + i;
    END_FOR;

END_PROGRAM