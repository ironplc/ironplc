FUNCTION_BLOCK MY_EXAMPLE
VAR
  long_STRING160              : STRING(160);
  sensor_bits                 : ARRAY [0..58]   OF BOOL;
END_VAR

VAR_INPUT
    DST_EN : BOOL;              (* daylight savings time enable *)
END_VAR

VAR CONSTANT
    ITEMS : ARRAY [0..3] OF INT := [
        (name := 'First', day := -2, month := 0, use := 1),
        (name := 'Last', day := 31, month := 12, use := 1)
    ];
END_VAR

    (* Test exponentiation operator *)
    x := y ** 2;

    (* Test MOD operator *)
    result := value MOD 10;

    (* Test comparison operators *)
    flag := a >= b;
    flag := a <= b;

END_FUNCTION_BLOCK
