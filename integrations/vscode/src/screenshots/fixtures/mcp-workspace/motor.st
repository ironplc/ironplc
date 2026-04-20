PROGRAM MotorControl
VAR
    StartButton : BOOL;
    StopButton : BOOL;
    MotorContactor : BOOL;
END_VAR

    MotorContactor := (StartButton OR MotorContactor) AND StopButton;

END_PROGRAM

CONFIGURATION config
    RESOURCE res ON PLC
        TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
        PROGRAM plc_task_instance WITH plc_task : MotorControl;
    END_RESOURCE
END_CONFIGURATION
