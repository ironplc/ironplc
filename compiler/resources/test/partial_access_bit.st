FUNCTION_BLOCK MyBlock
  VAR
    myWord : WORD;
    myBool : BOOL;
  END_VAR

  myBool := myWord.%X0;
  myBool := myWord.%X15;
END_FUNCTION_BLOCK
