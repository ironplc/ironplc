PROGRAM main
VAR
    counter : INT := 42;
    p : POINTER TO INT;
    value : INT;
END_VAR
    p := ADR(counter);
    value := p^;
    p^ := 99;
END_PROGRAM
