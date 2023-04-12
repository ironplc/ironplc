FUNCTION_BLOCK CounterLD
  VAR_INPUT
    Reset : BOOL;
  END_VAR
  VAR_OUTPUT
    Out : INT;
  END_VAR
  VAR
    Cnt : INT;
  END_VAR
  VAR_EXTERNAL CONSTANT
    ResetCounterValue : INT;
  END_VAR
  VAR
    _TMP_ADD4_OUT : INT;
    _TMP_SEL7_OUT : INT;
  END_VAR

  _TMP_ADD4_OUT := ADD(1, Cnt);
  _TMP_SEL7_OUT := SEL(Reset, _TMP_ADD4_OUT, ResetCounterValue);
  Cnt := _TMP_SEL7_OUT;
  Out := Cnt;
END_FUNCTION_BLOCK