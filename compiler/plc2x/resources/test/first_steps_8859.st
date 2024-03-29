(* ö *)
TYPE
  LOGLEVEL : (CRITICAL, WARNING, INFO, DEBUG) := INFO;
END_TYPE

FUNCTION_BLOCK LOGGER
  VAR_INPUT
    TRIG : BOOL;
    MSG : STRING;
    LEVEL : LOGLEVEL := INFO;
  END_VAR
  VAR
    TRIG0 : BOOL;
  END_VAR

  IF TRIG AND NOT TRIG0 THEN
  
  END_IF;
  TRIG0:=TRIG;
END_FUNCTION_BLOCK


FUNCTION_BLOCK CounterSFC
  VAR_INPUT
    Reset : BOOL;
  END_VAR
  VAR_OUTPUT
    OUT : INT;
  END_VAR
  VAR
    Cnt : INT;
  END_VAR
  VAR_EXTERNAL CONSTANT
    ResetCounterValue : INT;
  END_VAR

  INITIAL_STEP Start:
  END_STEP

  TRANSITION FROM Start TO ResetCounter
    := Reset;
  END_TRANSITION

  STEP ResetCounter:
    RESETCOUNTER_INLINE1(N);
    RESETCOUNTER_INLINE2(N);
  END_STEP

  ACTION RESETCOUNTER_INLINE1:
    Cnt := ResetCounterValue;
  END_ACTION

  ACTION RESETCOUNTER_INLINE2:
    OUT := Cnt;
  END_ACTION

  TRANSITION FROM ResetCounter TO Start
    := NOT Reset;
  END_TRANSITION

  TRANSITION FROM Start TO Count
    := NOT Reset;
  END_TRANSITION

  STEP Count:
    COUNT_INLINE3(N);
    COUNT_INLINE4(N);
  END_STEP

  ACTION COUNT_INLINE3:
    Cnt := Cnt + 1;
  END_ACTION

  ACTION COUNT_INLINE4:
    OUT := Cnt;
  END_ACTION

  TRANSITION FROM Count TO Start
    := Reset;
  END_TRANSITION

END_FUNCTION_BLOCK

FUNCTION_BLOCK CounterFBD
  VAR_INPUT
    Reset : BOOL;
  END_VAR
  VAR_OUTPUT
    OUT : INT;
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

  Cnt := _TMP_SEL7_OUT;
  OUT := Cnt;
END_FUNCTION_BLOCK


FUNCTION AverageVal : REAL
  VAR_INPUT
    Cnt1 : INT;
    Cnt2 : INT;
    Cnt3 : INT;
    Cnt4 : INT;
    Cnt5 : INT;
  END_VAR
  VAR
    InputsNumber : REAL := 5.0;
  END_VAR

  AverageVal := INT_TO_REAL(Cnt1+Cnt2+Cnt3+Cnt4+Cnt5)/InputsNumber;
END_FUNCTION

FUNCTION_BLOCK CounterST
  VAR_INPUT
    Reset : BOOL;
  END_VAR
  VAR
    Cnt : INT;
  END_VAR
  VAR_OUTPUT
    OUT : INT;
  END_VAR
  VAR_EXTERNAL CONSTANT
    ResetCounterValue : INT;
  END_VAR

  IF Reset THEN
    Cnt := ResetCounterValue;
  ELSE
    Cnt := Cnt + 1;
  END_IF;

  Out := Cnt;
END_FUNCTION_BLOCK


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

  Cnt := _TMP_SEL7_OUT;
  Out := Cnt;
END_FUNCTION_BLOCK

CONFIGURATION config
  VAR_GLOBAL CONSTANT
    ResetCounterValue : INT := 17;
  END_VAR

  RESOURCE resource1 ON PLC
    TASK plc_task(INTERVAL := T#100ms,PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : plc_prg;
  END_RESOURCE
END_CONFIGURATION

