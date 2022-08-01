PROGRAM plc_prg
  VAR_INPUT
    Reset : BOOL;
  END_VAR
  VAR_OUTPUT
    Cnt1 : INT;
    Cnt2 : INT;
    Cnt3 : INT;
    Cnt4 : INT;
    Cnt5 : INT;
  END_VAR
  VAR
    CounterST0 : CounterST;
    CounterFBD0 : CounterFBD;
    CounterSFC0 : CounterSFC;
    CounterIL0 : CounterIL;
    CounterLD0 : CounterLD;
    AVCnt : REAL;
    _TMP_AverageVal17_OUT : REAL;
  END_VAR

  CounterST0(Reset := Reset);
  Cnt1 := CounterST0.OUT;
  _TMP_AverageVal17_OUT := AverageVal(Cnt1, Cnt2, Cnt3, Cnt4, Cnt5);
  AVCnt := _TMP_AverageVal17_OUT;
  CounterFBD0(Reset := Reset);
  Cnt2 := CounterFBD0.OUT;
  CounterSFC0(Reset := Reset);
  Cnt3 := CounterSFC0.OUT;
  CounterIL0(Reset := Reset);
  Cnt4 := CounterIL0.OUT;
  CounterLD0(Reset := Reset);
  Cnt5 := CounterLD0.Out;
END_PROGRAM
