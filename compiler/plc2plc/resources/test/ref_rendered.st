TYPE
   IntRef : REF_TO INT ;
END_TYPE
TYPE
   ArrRef : REF_TO ARRAY [ 0.. 3 ] OF DINT ;
END_TYPE
FUNCTION read_ref : INT
   VAR_INPUT
      pt : REF_TO INT;
   END_VAR

   read_ref := pt ^ ;
END_FUNCTION
PROGRAM main

VAR
   counter : INT := 42;
END_VAR

VAR
   r : REF_TO INT := REF( counter );
END_VAR

VAR
   r_null : REF_TO INT := NULL;
END_VAR

VAR
   r_uninit : REF_TO INT;
END_VAR

VAR
   data : REF_TO ARRAY [ 0.. 3 ] OF INT;
END_VAR

VAR
   refs : ARRAY [ 0.. 2 ] OF REF_TO INT;
END_VAR

VAR
   value : INT;
END_VAR
value := r ^ ;
r^ := 99 ;
r := REF( counter ) ;
IF ( r <> NULL ) THEN
   value := r ^ ;
END_IF ;

value := read_ref ( pt := REF( counter ) ) ;
END_PROGRAM
