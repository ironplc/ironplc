(* End-to-end installer test fixture.

   This program depends on the bundled `Tc2_System` compatibility library: `PI`
   is a global constant that the library provides, not a language built-in. The
   program therefore compiles only if the installer shipped the library files
   (`resources/libs/Tc2_System/...`) beside the compiler binary and the compiler
   resolves them from that installed location.

   Compile with:
     ironplcc check --dialect twincat \
       --allow-constant-initializer-expressions \
       --library Tc2_System tests/e2e/library/uses_pi.st

   `--dialect twincat` permits the library's top-level VAR_GLOBAL, and
   `--allow-constant-initializer-expressions` permits the `PI / 180.0`
   initializer. See the `library-e2e` recipe in the repository-root justfile. *)
FUNCTION_BLOCK FB_Angle
    VAR
        degrees_to_radians : LREAL := PI / 180.0;
    END_VAR
END_FUNCTION_BLOCK
