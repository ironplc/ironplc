PROGRAM main

VAR
   a : WSTRING [ 10 ] :="foo";
END_VAR

VAR
   b : WSTRING [ 10 ] :="bar";
END_VAR

VAR
   c : WSTRING [ 20 ];
END_VAR

VAR
   n : DINT;
END_VAR

VAR
   eq : BOOL;
END_VAR
c := a ;
c := CONCAT ( a , b ) ;
n := LEN ( a ) ;
eq := ( a = b ) ;
END_PROGRAM
