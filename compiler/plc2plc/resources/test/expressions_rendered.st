FUNCTION fun : BOOL
   VAR_INPUT
      var1 : BYTE;
   END_VAR

   IF ( var1 < BYTE#32 ) THEN
      var1 := 1 ;
   END_IF ;

   var1 := NOT ( var3 OR var3 ) ;
   var1 := FUNC_NAME ( var1 , '  ' ) ;
   var1 := ( ( - 1 + ( 2 * 3 ) ) - ( ( 4 / 6 ) MOD ( 7 ** 8 ) ) ) ;
   var1 := ( ( ( ( 1 >= 2 ) <= 3 ) <> 4 ) XOR 5 ) ;
   IF ( var1 > TIME#1000ms ) THEN
      var1 := 1 ;
   END_IF ;

END_FUNCTION
