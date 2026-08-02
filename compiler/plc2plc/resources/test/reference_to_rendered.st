TYPE
   IntRef : REFERENCE TO INT ;
END_TYPE
PROGRAM main

VAR
   counter : INT := 42;
END_VAR

VAR
   r : REFERENCE TO INT;
END_VAR

VAR
   refs : ARRAY [ 0.. 2 ] OF REFERENCE TO INT;
END_VAR

VAR
   value : INT;
END_VAR
r REF= counter ;
value := r ^ ;
r^ := 99 ;
refs [ 0 ] REF= counter ;
value := refs [ 0 ] ^ ;
END_PROGRAM
