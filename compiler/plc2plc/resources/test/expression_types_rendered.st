TYPE
   Color : ( Red , Green , Blue ) ;
END_TYPE
FUNCTION Scale : LREAL
   VAR_INPUT
      raw : dint;
   END_VAR

   VAR_INPUT
      gain : lreal;
   END_VAR

   Scale := ( raw (* DINT -> LREAL *) * gain (* LREAL *) ) (* LREAL *) ;
END_FUNCTION
PROGRAM main

VAR
   count : int;
END_VAR

VAR
   total : dint;
END_VAR

VAR
   big : lint;
END_VAR

VAR
   level : real;
END_VAR

VAR
   out : lreal;
END_VAR

VAR
   flag : bool;
END_VAR

VAR
   arr : ARRAY [ 1.. 4 ] OF INT;
END_VAR

VAR
   i : int;
END_VAR

VAR
   c : Color;
END_VAR

VAR
   elapsed : time;
END_VAR

VAR
   name : STRING [ 20 ];
END_VAR

VAR
   r : REF_TO INT;
END_VAR
count := ( count (* INT *) + 1 (* CONSTANT INT *) ) (* INT *) ;
flag := ( big (* LINT *) > total (* DINT -> LINT *) ) (* BOOL *) ;
level := count (* INT -> REAL *) ;
out := Scale ( total (* DINT *) , 0.5 (* CONSTANT LREAL *) ) (* LREAL *) ;
FOR i := 1 (* CONSTANT INT *) TO 4 (* CONSTANT INT *) DO
   arr[ i (* INT *) ] := MAX ( arr[ i (* INT *) ] (* INT *) , count (* INT *) ) (* INT *) ;
END_FOR ;

IF ( ( NOT flag (* BOOL *) ) (* BOOL *) AND ( total (* DINT *) < 10 (* CONSTANT DINT *) ) (* BOOL *) ) (* BOOL *) THEN
   big := ( ABS ( big (* LINT *) ) (* LINT *) + total (* DINT -> LINT *) ) (* LINT *) ;
END_IF ;

c := Green (* CONSTANT Color *) ;
flag := ( c (* Color *) = Red (* CONSTANT Color *) ) (* BOOL *) ;
elapsed := ( elapsed (* TIME *) + TIME#1000ms (* CONSTANT TIME *) ) (* TIME *) ;
name := CONCAT ( name (* STRING *) , 'abc' (* CONSTANT STRING *) ) (* STRING *) ;
r := REF( count ) (* REF_TO INT *) ;
count := ( ( r (* REF_TO INT *)^ ) (* INT *) + 1 (* CONSTANT INT *) ) (* INT *) ;
r := NULL (* CONSTANT NULL *) ;
count := 12 (* CONSTANT INT *) ;
END_PROGRAM
