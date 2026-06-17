PROGRAM main
    VAR
        a : WSTRING[10] := "foo";
        b : WSTRING[10] := "bar";
        c : WSTRING[20];
        n : DINT;
        eq : BOOL;
    END_VAR
    c := a;
    c := CONCAT(a, b);
    n := LEN(a);
    eq := a = b;
END_PROGRAM
