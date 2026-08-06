(* End-to-end installer test fixture.

   A minimal runnable program that depends on the bundled `Tc2_System`
   compatibility library: `PI` is a global constant the library provides, not a
   language built-in. The program compiles and runs only if the installer
   shipped the library files (`resources/libs/Tc2_System/...`) beside the
   compiler binary and the compiler resolves them from that installed location.

   Compile, then run one scan and dump the variables:
     ironplcc compile --dialect twincat --library Tc2_System \
       --output prog.iplc tests/e2e/library/uses_pi.st
     ironplcvm run prog.iplc --scans 1 --dump-vars -

   After one scan `circumference` holds 2 * PI * 10.0 = 62.83185307179586, so the
   dump proves the library `PI` resolved and the VM computed with it end to end.
   `--dialect twincat` permits the library's top-level VAR_GLOBAL. See the
   `library-e2e` recipe in the repository-root justfile. *)
PROGRAM main
    VAR
        circumference : LREAL;
    END_VAR
    circumference := 2.0 * PI * 10.0;
END_PROGRAM
