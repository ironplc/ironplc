PROGRAM main

VAR
   counter : INT := 42;
END_VAR

VAR
   p : POINTER TO INT;
END_VAR

VAR
   value : INT;
END_VAR
p := ADR ( counter ) ;
value := p ^ ;
p^ := 99 ;
END_PROGRAM
