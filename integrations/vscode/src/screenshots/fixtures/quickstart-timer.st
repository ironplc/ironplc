PROGRAM main
   VAR
      Button : BOOL;
      Buzzer : BOOL;
      PulseTimer : TON;
   END_VAR

   PulseTimer(IN := NOT Button, PT := T#500ms);
   Buzzer := PulseTimer.Q;

END_PROGRAM

CONFIGURATION config
   RESOURCE res ON PLC
      TASK plc_task(INTERVAL := T#100ms, PRIORITY := 1);
      PROGRAM plc_task_instance WITH plc_task : main;
   END_RESOURCE
END_CONFIGURATION
