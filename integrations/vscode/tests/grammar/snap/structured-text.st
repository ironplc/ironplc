(* Block comment test *)
// Line comment test

PROGRAM TestProgram
VAR
    myBool : BOOL := TRUE;
    myInt : INT := 42;
    myReal : REAL := 3.14;
    myString : STRING := 'Hello';
    myArray : ARRAY[0..9] OF DINT;
    address1 : BOOL AT %IX0.0;
    address2 : WORD AT %QW10;
END_VAR

VAR_INPUT
    input1 : SINT;
    input2 : UDINT;
END_VAR

VAR_OUTPUT
    output1 : LINT;
END_VAR

VAR_TEMP
    temp1 : LREAL;
END_VAR

    (* Conditional statements *)
    IF input1 > 0 THEN
        output1 := input1 * 2;
    ELSIF input1 < 0 THEN
        output1 := -input1;
    ELSE
        output1 := 0;
    END_IF;

    (* Case statement *)
    CASE myInt OF
        1: myBool := TRUE;
        2, 3: myBool := FALSE;
    ELSE
        myBool := NOT myBool;
    END_CASE;

    (* For loop *)
    FOR i := 0 TO 9 BY 1 DO
        myArray[i] := i * i;
    END_FOR;

    (* While loop *)
    WHILE myInt > 0 DO
        myInt := myInt - 1;
    END_WHILE;

    (* Repeat loop *)
    REPEAT
        temp1 := temp1 + 1.0;
    UNTIL temp1 >= 10.0
    END_REPEAT;

    (* Operators *)
    myBool := (input1 = input2) OR (input1 <> 0);
    myBool := myBool AND NOT FALSE;
    myBool := input1 < input2;
    myBool := input1 >= input2;

    (* Numeric literals *)
    myInt := 16#FF;
    myInt := 8#77;
    myInt := 2#1010;
    myReal := 1.5e10;

END_PROGRAM

FUNCTION MyFunction : BOOL
VAR_INPUT
    x : INT;
END_VAR
    IF x > 0 THEN
        RETURN TRUE;
    ELSE
        RETURN FALSE;
    END_IF;
END_FUNCTION

FUNCTION_BLOCK MyFB
VAR
    counter : UDINT;
END_VAR
    counter := counter + 1;
END_FUNCTION_BLOCK

TYPE MyStruct :
STRUCT
    field1 : BOOL;
    field2 : INT;
END_STRUCT
END_TYPE
