PROGRAM fb
    VAR CONSTANT
        E : REAL := 2.718 ;
    END_VAR

    VAR
        AT %IX1.1 : BOOL;
        AT %IX2.1 : BOOL :=1;
        AT %MW1 : INT := 2 ;
    END_VAR

    VAR RETAIN
       AT %QW1 : WORD := 16#FF00 ;
       AT %QB2 : WORD := 8#7700 ;
       AT %QB3 : WORD := 2#1010 ;
       C2 AT %Q* : BYTE ;
       C3 AT %MW1.2.3 : WORD;
    END_VAR

    VAR
        XWORD, YWORD, ZWORD : INT;

        UNINIT: ARRAY [0..9] OF INT;
        INIT : ARRAY [1..12] OF SINT := [2,7,18];
        MULTI_DIM_ARRAY : ARRAY[1..2,3..4,5..6] OF SINT;
        PARENTHESIS_ARRAY : ARRAY[0..3] OF SINT := [3(2)];

        VAL_STR: STRING[2];
        VAL_STR_INIT: STRING := 'abc';
        VAL_STR_SIZED_INIT: STRING[3] := 'abc';
        VAL_WSTR: WSTRING[9];
        VAL_WSTR_INIT: WSTRING := "def";
        VAL_WSTR_SIZED_INIT: WSTRING[3] := "def";
    END_VAR

    VAR
        NAME : ONE_STRUCT_ELEMENT := (ELEM := VAL);
        NAME2 : TWO_STRUCT_ELEMENTS := (ELEM1 := VAL1, ELEM2 := VAL2);
        NAME3 : STRUCT_TWO_ELEMENTS := (ELEM1 := VAL1, ELEM2 := VAL2);
        (* TODO Nested structures *)
    END_VAR

END_PROGRAM