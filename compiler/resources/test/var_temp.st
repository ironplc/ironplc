FUNCTION my_func : DINT
  VAR_INPUT
    a : DINT;
  END_VAR
  VAR_TEMP
    temp : DINT;
  END_VAR
  temp := a * 2;
  my_func := temp;
END_FUNCTION

FUNCTION_BLOCK my_fb
  VAR_TEMP
    t : INT;
  END_VAR
  t := 42;
END_FUNCTION_BLOCK

PROGRAM main
  VAR
    q : INT;
  END_VAR
  VAR_TEMP
    t : INT := 3;
  END_VAR
  q := t;
END_PROGRAM
