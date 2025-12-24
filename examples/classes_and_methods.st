(*
    Example demonstrating object-oriented programming with
    classes and methods in IronPLC.
*)

// Motor control class with state management
CLASS Motor
    VAR
        speed : REAL := 0.0;
        running : BOOL := FALSE;
        direction : BOOL := TRUE;  // TRUE = forward, FALSE = reverse
        max_speed : REAL := 3000.0;
    END_VAR
    
    METHOD Start : BOOL
        IF NOT running THEN
            running := TRUE;
            Start := TRUE;
        ELSE
            Start := FALSE;  // Already running
        END_IF;
    END_METHOD
    
    METHOD Stop
        running := FALSE;
        speed := 0.0;
    END_METHOD
    
    METHOD SetSpeed : BOOL
        VAR_INPUT
            target_speed : REAL;
        END_VAR
        
        IF running AND (target_speed >= 0.0) AND (target_speed <= max_speed) THEN
            speed := target_speed;
            SetSpeed := TRUE;
        ELSE
            SetSpeed := FALSE;
        END_IF;
    END_METHOD
    
    METHOD Reverse
        direction := NOT direction;
    END_METHOD
    
    METHOD GetStatus : DWORD
        VAR
            status : DWORD := 0;
        END_VAR
        
        IF running THEN
            status := status OR 16#0001;  // Running bit
        END_IF;
        
        IF direction THEN
            status := status OR 16#0002;  // Direction bit
        END_IF;
        
        GetStatus := status;
    END_METHOD
END_CLASS

// Temperature controller class
CLASS TemperatureController
    VAR
        setpoint : REAL := 20.0;
        current_temp : REAL := 0.0;
        output : REAL := 0.0;
        kp : REAL := 1.0;  // Proportional gain
        enabled : BOOL := FALSE;
    END_VAR
    
    METHOD Enable
        enabled := TRUE;
    END_METHOD
    
    METHOD Disable
        enabled := FALSE;
        output := 0.0;
    END_METHOD
    
    METHOD SetSetpoint
        VAR_INPUT
            new_setpoint : REAL;
        END_VAR
        
        setpoint := new_setpoint;
    END_METHOD
    
    METHOD UpdateTemperature
        VAR_INPUT
            measured_temp : REAL;
        END_VAR
        VAR
            error : REAL;
        END_VAR
        
        current_temp := measured_temp;
        
        IF enabled THEN
            error := setpoint - current_temp;
            output := kp * error;
            
            // Limit output to 0-100%
            IF output > 100.0 THEN
                output := 100.0;
            ELSIF output < 0.0 THEN
                output := 0.0;
            END_IF;
        END_IF;
    END_METHOD
END_CLASS

PROGRAM ObjectOrientedDemo
    VAR
        main_motor : Motor;
        aux_motor : Motor;
        temp_ctrl : TemperatureController;
        motor_started : BOOL;
        speed_set : BOOL;
        motor_status : DWORD;
        room_temperature : REAL := 18.5;
    END_VAR
    
    // Motor control demonstration
    motor_started := main_motor.Start();
    
    IF motor_started THEN
        speed_set := main_motor.SetSpeed(1500.0);
        motor_status := main_motor.GetStatus();
        
        // Reverse direction after some condition
        main_motor.Reverse();
    END_IF;
    
    // Temperature controller demonstration
    temp_ctrl.Enable();
    temp_ctrl.SetSetpoint(22.0);
    temp_ctrl.UpdateTemperature(room_temperature);
    
    // Multiple motor instances
    aux_motor.Start();
    aux_motor.SetSpeed(800.0);
END_PROGRAM