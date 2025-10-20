CONFIGURATION config
   RESOURCE resource1 ON PLC
      TASK plc_task ( INTERNAL := TIME#100ms , PRIORITY := 1 ) ;
      PROGRAM plc_task_instance WITH plc_task : plc_prg ;
   END_RESOURCE
   VAR_CONFIG
Some.FunctionBlock.Item.Path : FB_TYPE_NAME := ( ELEM := VAL );
   END_VAR
   VAR_CONFIG
Some.Located.Item.Path AT %QB1 : BYTE ;
   END_VAR

END_CONFIGURATION
PROGRAM main

VAR
   Button AT %IX1 : BOOL;
END_VAR

VAR
   Buzzer AT %QX1 : BOOL;
END_VAR
Buzzer := NOT Button ;
END_PROGRAM
