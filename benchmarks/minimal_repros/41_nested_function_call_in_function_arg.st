(* Minimal reproduction: nested stdlib function call as argument.

   Calling a standard string function like FIND where one argument
   is itself a function call (e.g. FIND(str, MID(s, 1, n))) previously
   failed in codegen because resolve_string_arg only handled variables
   and string literals.

   Compile with:
     ironplcc compile benchmarks/minimal_repros/41_nested_function_call_in_function_arg.st
*)

FUNCTION MY_SEARCH : INT
VAR_INPUT
    haystack : STRING[255];
    needle : STRING[255];
    maxlen : INT;
END_VAR
    MY_SEARCH := FIND(haystack, MID(needle, 1, maxlen));
END_FUNCTION

PROGRAM main
VAR
    r : INT;
END_VAR
    r := MY_SEARCH(haystack := 'hello world', needle := 'world', maxlen := 3);
END_PROGRAM
