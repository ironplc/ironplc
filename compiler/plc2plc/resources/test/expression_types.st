TYPE
    Color : (Red, Green, Blue);
END_TYPE

FUNCTION Scale : LREAL
VAR_INPUT
    raw : DINT;
    gain : LREAL;
END_VAR
    Scale := raw * gain;
END_FUNCTION

PROGRAM main
VAR
    count : INT;
    total : DINT;
    big : LINT;
    level : REAL;
    out : LREAL;
    flag : BOOL;
    arr : ARRAY[1..4] OF INT;
    i : INT;
    c : Color;
    elapsed : TIME;
    name : STRING[20];
    r : REF_TO INT;
END_VAR
    count := count + 1;
    flag := big > total;
    level := count;
    out := Scale(raw := total, gain := 0.5);
    FOR i := 1 TO 4 DO
        arr[i] := MAX(arr[i], count);
    END_FOR;
    IF NOT flag AND total < 10 THEN
        big := ABS(big) + total;
    END_IF;
    c := Green;
    flag := c = Red;
    elapsed := elapsed + T#1s;
    name := CONCAT(name, 'abc');
    r := REF(count);
    count := r^ + 1;
    r := NULL;
    count := INT#5 + 7;
END_PROGRAM
