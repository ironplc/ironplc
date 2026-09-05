(* End-to-end installer test fixture.

   A minimal runnable program that depends on the bundled `Tc2_BuiltIns`
   compatibility library: `BOOL_TO_STRING` is a TwinCAT built-in conversion
   operator the library provides, not an IronPLC language built-in. The program
   compiles and runs only if the installer shipped the library files
   (`resources/libs/Tc2_BuiltIns/...`) beside the compiler binary and the
   compiler resolves them from that installed location.

   Compile, then run one scan and dump the variables:
     ironplcc compile --dialect twincat --library Tc2_BuiltIns \
       --output prog.iplc tests/e2e/library/uses_bool_to_string.st
     ironplcvm run prog.iplc --scans 1 --dump-vars -

   `--dump-vars` does not render STRING contents, so the results are checked
   through string comparisons into BOOL variables: after one scan `okTrue` and
   `okFalse` are both TRUE, proving the library function returned 'TRUE' and
   'FALSE' end to end. See the `library-e2e` recipe in the repository-root
   justfile. *)
PROGRAM main
    VAR
        statusTrue : STRING;
        statusFalse : STRING;
        okTrue : BOOL;
        okFalse : BOOL;
    END_VAR
    statusTrue := BOOL_TO_STRING(TRUE);
    statusFalse := BOOL_TO_STRING(FALSE);
    okTrue := statusTrue = 'TRUE';
    okFalse := statusFalse = 'FALSE';
END_PROGRAM
