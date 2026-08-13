TYPE
   IntPtr : POINTER TO INT ;
END_TYPE
PROGRAM main

VAR
   counter : INT := 42;
END_VAR

VAR
   p : POINTER TO INT;
END_VAR

VAR
   ptrs : ARRAY [ 0.. 2 ] OF POINTER TO INT;
END_VAR

VAR
   value : INT;
END_VAR
p := REF( counter ) ;
value := p ^ ;
p^ := 99 ;
p := NULL ;
END_PROGRAM
