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
    END_VAR

END_PROGRAM