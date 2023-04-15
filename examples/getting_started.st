(*
    This example is part of IronPLC and is intended
    as a quick check of IronPLC's capabilities. It
    is not intended to be a useful program.
*)

(*
    The program increments an internal that each time the
    program task executes.
*)
PROGRAM plc_counter
    VAR
        Counter: INT;
    END_VAR

    Counter := Counter + 1;
END_PROGRAM

CONFIGURATION config
  RESOURCE plc_res ON PLC
    TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM plc_task_instance WITH plc_task : plc_counter;
  END_RESOURCE
END_CONFIGURATION