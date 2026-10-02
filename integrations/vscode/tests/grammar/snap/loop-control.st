PROGRAM main
VAR
    i : INT;
END_VAR
    FOR i := 1 TO 10 DO
        IF i = 2 THEN
            CONTINUE;
        END_IF;
        IF i = 8 THEN
            EXIT;
        END_IF;
    END_FOR;
END_PROGRAM
