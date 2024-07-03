FUNCTION AverageVal : REAL
    VAR_INPUT
    Cnt1 : INT;
    Cnt2 : INT;
    Cnt3 : INT;
    Cnt4 : INT;
    Cnt5 : INT;
  END_VAR
    VAR
    InputsNumber : REAL := 5.1;
  END_VAR
    AverageVal := INT_TO_REAL(Cnt1+Cnt2+Cnt3+Cnt4+Cnt5)/InputsNumber;
    END_FUNCTION