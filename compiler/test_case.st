PROGRAM Test
VAR
    state : INT;
    result : INT;
END_VAR

CASE state OF
    1: result := 10;
    2: result := 20;
ELSE
    result := 0;
END_CASE

END_PROGRAM
