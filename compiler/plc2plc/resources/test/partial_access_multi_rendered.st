PROGRAM main

VAR
   d : DWORD;
END_VAR

VAR
   l : LWORD;
END_VAR

VAR
   rb : BYTE;
END_VAR

VAR
   rw : WORD;
END_VAR

VAR
   rd : DWORD;
END_VAR
rb := d.%B0 ;
rw := d.%W1 ;
rd := l.%D0 ;
d.%B2 := BYTE#255 ;
l.%W3 := WORD#43981 ;
END_PROGRAM
