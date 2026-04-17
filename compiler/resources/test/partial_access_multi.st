PROGRAM main
  VAR
    d : DWORD;
    l : LWORD;
    rb : BYTE;
    rw : WORD;
    rd : DWORD;
  END_VAR
  rb := d.%B0;
  rw := d.%W1;
  rd := l.%D0;
  d.%B2 := BYTE#16#FF;
  l.%W3 := WORD#16#ABCD;
END_PROGRAM
