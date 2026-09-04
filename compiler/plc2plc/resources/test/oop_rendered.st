INTERFACE I_Loggable
END_INTERFACE
INTERFACE I_Drivable EXTENDS I_Loggable
END_INTERFACE
FUNCTION_BLOCK ABSTRACT FB_BaseMotor IMPLEMENTS I_Drivable

   VAR
      bRunning : BOOL;
   END_VAR

METHOD Start
   bRunning := BOOL#TRUE ;
END_METHOD
METHOD Stop
   bRunning := BOOL#FALSE ;
END_METHOD
END_FUNCTION_BLOCK
FUNCTION_BLOCK FB_ServoMotor EXTENDS FB_BaseMotor IMPLEMENTS I_Drivable

   VAR
      rSpeed : REAL;
   END_VAR

METHOD SetSpeed : BOOL

   VAR_INPUT
      rNewSpeed : REAL;
   END_VAR

   SUPER^. Start ( ) ;
   rSpeed := rNewSpeed ;
   SetSpeed := BOOL#TRUE ;
END_METHOD
METHOD Reset
   THIS^.rSpeed := 0.0 ;
   SUPER^. Stop ( ) ;
END_METHOD
END_FUNCTION_BLOCK
PROGRAM main

VAR
   m : FB_ServoMotor;
END_VAR
m. SetSpeed ( 1.5 ) ;
m. SetSpeed ( rNewSpeed := 2.0 ) ;
m. Reset ( ) ;
END_PROGRAM
