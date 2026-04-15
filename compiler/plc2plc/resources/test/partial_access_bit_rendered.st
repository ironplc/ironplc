FUNCTION_BLOCK MyBlock

   VAR
      myWord : WORD;
   END_VAR

   VAR
      myBool : BOOL;
   END_VAR
   myBool := myWord.0 ;
   myBool := myWord.15 ;

END_FUNCTION_BLOCK
