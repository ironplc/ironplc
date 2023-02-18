FUNCTION_BLOCK BLOCK
    NUM_INT := 10;
    NUM_FLOAT := 10.0;

    IF NUM1 < NUM2 THEN
        NUM1 := 1;
    ELSIF NUM2 = NUM3 THEN
        NUM1 := 2;
    ELSE
        NUM1 := 3;
    END_IF;

    CASE EXPR_WITH_ELSE OF
    VAL1: (* empty *);
    VAL2: NUM1 := 1;
    ELSE NUM2 := 2;
    END_CASE;

    CASE EXPR_NO_ELSE OF
    VAL1: (* empty *);
    VAL2: NUM1 := 1;
    END_CASE;

END_FUNCTION_BLOCK