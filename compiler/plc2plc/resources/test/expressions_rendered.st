 FUNCTION fun : BOOL
   VAR_INPUT
      var1 : BYTE;
   END_VAR

   IF ( var1 < BYTE#32 ) THEN
      var1 := 1 ;
   END_IF ;

   var1 := NOT ( var3 OR var3 ) ;
   var1 := FUNC_NAME ( var1 , '  ' ) ;
   IF ( var1 > TIME#1000ms ) THEN
      var1 := 1 ;
   END_IF ;

END_FUNCTION
