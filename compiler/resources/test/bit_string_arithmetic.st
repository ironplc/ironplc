(* Arithmetic on a bit-string operand: accepted only with
   --allow-bit-string-arithmetic or a dialect that enables it. *)
PROGRAM main
  VAR
    counter : BYTE;
  END_VAR
  counter := counter + 1;
END_PROGRAM
