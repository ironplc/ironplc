PROGRAM main
VAR
  x : INT;
  y : DWORD;
  arr : ARRAY[1..10] OF INT;
  s : DINT;
END_VAR
  s := SIZEOF(x);
  s := SIZEOF(y);
  s := SIZEOF(arr);
END_PROGRAM
