TYPE
    MyString : STRING(25);
    LongString : STRING(255);
END_TYPE

PROGRAM TestStringTypes
VAR
    shortText : MyString := 'hello';
    longText : LongString;
END_VAR

shortText := 'world';
longText := 'this is a much longer string that fits in 255 characters';

END_PROGRAM