(* Simple comment *)

PROGRAM TestProgram
VAR
    x : BOOL;
    timer1 : TON;
    myString : STRING(50);
END_VAR

timer1(IN := TRUE, PT := T#5S);
x := timer1.Q;

END_PROGRAM