TYPE IntPtr : POINTER TO INT; END_TYPE

PROGRAM main
VAR
    counter : INT := 42;
    p : POINTER TO INT;
    ptrs : ARRAY[0..2] OF POINTER TO INT;
    value : INT;
END_VAR
    p := REF(counter);
    value := p^;
    p^ := 99;
    p := NULL;
END_PROGRAM
