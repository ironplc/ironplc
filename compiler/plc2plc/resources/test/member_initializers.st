TYPE
   Color : ( Red , Green ) ;
END_TYPE
TYPE
   Inner : STRUCT
      c : Color ;
   END_STRUCT ;
END_TYPE
TYPE
   Outer : STRUCT
      values : ARRAY [ 1.. 2 ] OF INT ;
      inner : Inner ;
   END_STRUCT ;
END_TYPE
FUNCTION_BLOCK Lamp

   VAR_INPUT
      color : Color;
   END_VAR

END_FUNCTION_BLOCK
PROGRAM main

VAR
   o : Outer := ( values := [ 1 , 2 ] , inner := ( c := Green ) );
END_VAR

VAR
   lamp : Lamp := ( color := Red );
END_VAR
END_PROGRAM
