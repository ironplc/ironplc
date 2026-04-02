PROGRAM main

VAR
   x : INT;
END_VAR

VAR
   y : DWORD;
END_VAR

VAR
   arr : ARRAY [ 1.. 10 ] OF INT;
END_VAR

VAR
   s : DINT;
END_VAR
s := SIZEOF ( x ) ;
s := SIZEOF ( y ) ;
s := SIZEOF ( arr ) ;
END_PROGRAM
