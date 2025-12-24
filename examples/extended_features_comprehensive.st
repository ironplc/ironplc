(*
    Comprehensive example demonstrating all extended syntax features
    working together in a realistic industrial control scenario.
*)

// External system functions
{external}
FUNCTION GetSystemTime : DWORD
END_FUNCTION

@EXTERNAL
FUNCTION LogEvent : BOOL
    VAR_INPUT
        level : INT;
        message : STRING;
    END_VAR
END_FUNCTION

{external}
FUNCTION ReadSensorValue : REAL
    VAR_INPUT
        sensor_id : INT;
    END_VAR
END_FUNCTION

// Type definitions with range constraints
TYPE
    Temperature : REAL(-50.0..200.0);      // Temperature range in Celsius
    Pressure : REAL(0.0..1000.0);          // Pressure in bar
    FlowRate : REAL(0.0..500.0);           // Flow rate in L/min
    Percentage : DINT(0..100);              // Percentage values
    
    // Reference types for sensor data
    TempRef : REF_TO Temperature;
    PressureRef : REF_TO Pressure;
    
    // Sensor data structure
    SensorData : STRUCT
        temperature : Temperature;
        pressure : Pressure;
        flow_rate : FlowRate;
        timestamp : DWORD;
        valid : BOOL;
    END_STRUCT;
    
    SensorRef : REF_TO SensorData;
END_TYPE

// Process control class with comprehensive functionality
CLASS ProcessController
    VAR
        // Process variables
        setpoint_temp : Temperature := 75.0;
        setpoint_pressure : Pressure := 5.0;
        current_data : SensorData;
        
        // Control parameters
        temp_output : Percentage := 0;
        pressure_output : Percentage := 0;
        
        // Control gains (PID parameters)
        temp_kp : REAL := 1.0;
        temp_ki : REAL := 0.1;
        temp_kd : REAL := 0.05;
        
        pressure_kp : REAL := 2.0;
        pressure_ki : REAL := 0.2;
        
        // Internal state
        temp_integral : REAL := 0.0;
        temp_last_error : REAL := 0.0;
        pressure_integral : REAL := 0.0;
        
        enabled : BOOL := FALSE;
        alarm_active : BOOL := FALSE;
    END_VAR
    
    METHOD Initialize : BOOL
        // Reset all control variables
        temp_output := 0;
        pressure_output := 0;
        temp_integral := 0.0;
        temp_last_error := 0.0;
        pressure_integral := 0.0;
        alarm_active := FALSE;
        
        // Log initialization
        Initialize := LogEvent(1, 'Process controller initialized');
    END_METHOD
    
    METHOD Enable
        enabled := TRUE;
        LogEvent(1, 'Process controller enabled');
    END_METHOD
    
    METHOD Disable
        enabled := FALSE;
        temp_output := 0;
        pressure_output := 0;
        LogEvent(1, 'Process controller disabled');
    END_METHOD
    
    METHOD UpdateSensorData
        VAR_INPUT
            {ref} sensor_data : SensorData;
        END_VAR
        
        // Update internal sensor data
        current_data := sensor_data;
        current_data.timestamp := GetSystemTime();
        
        // Validate sensor readings
        IF (current_data.temperature < -50.0) OR (current_data.temperature > 200.0) THEN
            current_data.valid := FALSE;
            alarm_active := TRUE;
            LogEvent(3, 'Temperature sensor out of range');
        ELSIF (current_data.pressure < 0.0) OR (current_data.pressure > 1000.0) THEN
            current_data.valid := FALSE;
            alarm_active := TRUE;
            LogEvent(3, 'Pressure sensor out of range');
        ELSE
            current_data.valid := TRUE;
            alarm_active := FALSE;
        END_IF;
    END_METHOD
    
    METHOD CalculateControl
        VAR
            temp_error, temp_derivative : REAL;
            pressure_error : REAL;
            dt : REAL := 0.1;  // 100ms cycle time
        END_VAR
        
        IF enabled AND current_data.valid THEN
            // Temperature PID control
            temp_error := setpoint_temp - current_data.temperature;
            temp_integral := temp_integral + temp_error * dt;
            temp_derivative := (temp_error - temp_last_error) / dt;
            
            temp_output := REAL_TO_DINT(temp_kp * temp_error + 
                                       temp_ki * temp_integral + 
                                       temp_kd * temp_derivative);
            
            // Limit temperature output
            IF temp_output > 100 THEN
                temp_output := 100;
            ELSIF temp_output < 0 THEN
                temp_output := 0;
            END_IF;
            
            temp_last_error := temp_error;
            
            // Pressure PI control
            pressure_error := setpoint_pressure - current_data.pressure;
            pressure_integral := pressure_integral + pressure_error * dt;
            
            pressure_output := REAL_TO_DINT(pressure_kp * pressure_error + 
                                          pressure_ki * pressure_integral);
            
            // Limit pressure output
            IF pressure_output > 100 THEN
                pressure_output := 100;
            ELSIF pressure_output < 0 THEN
                pressure_output := 0;
            END_IF;
        ELSE
            temp_output := 0;
            pressure_output := 0;
        END_IF;
    END_METHOD
    
    METHOD SetTemperatureSetpoint : BOOL
        VAR_INPUT
            new_setpoint : Temperature;
        END_VAR
        
        IF (new_setpoint >= -50.0) AND (new_setpoint <= 200.0) THEN
            setpoint_temp := new_setpoint;
            SetTemperatureSetpoint := TRUE;
            LogEvent(1, 'Temperature setpoint updated');
        ELSE
            SetTemperatureSetpoint := FALSE;
            LogEvent(2, 'Invalid temperature setpoint');
        END_IF;
    END_METHOD
    
    METHOD GetStatus : DWORD
        VAR
            status : DWORD := 0;
        END_VAR
        
        IF enabled THEN
            status := status OR 16#0001;
        END_IF;
        
        IF current_data.valid THEN
            status := status OR 16#0002;
        END_IF;
        
        IF alarm_active THEN
            status := status OR 16#0004;
        END_IF;
        
        GetStatus := status;
    END_METHOD
END_CLASS

// Main control program with state machine using action blocks
PROGRAM IndustrialProcessControl
    VAR
        // Process controller instance
        controller : ProcessController;
        
        // Sensor data and references
        sensor_readings : ARRAY[1..3] OF SensorData;
        current_sensor : SensorRef;
        temp_ptr : TempRef;
        pressure_ptr : PressureRef;
        
        // State machine variables
        system_state : INT := 0;
        state_timer : TON;
        cycle_counter : DWORD := 0;
        
        // Process variables
        process_running : BOOL := FALSE;
        startup_complete : BOOL := FALSE;
        shutdown_requested : BOOL := FALSE;
        emergency_stop : BOOL := FALSE;
        
        // Data logging
        log_buffer : ARRAY[1..100] OF SensorData;
        log_index : INT := 1;
        log_ptr : SensorRef;
        
        // Statistics
        avg_temperature : REAL := 0.0;
        max_pressure : Pressure := 0.0;
        
        i : INT;
    END_VAR
    
    // Main state machine
    CASE system_state OF
        0: SystemInitialization();
        1: SensorCalibration();
        2: ProcessStartup();
        3: NormalOperation();
        4: ProcessShutdown();
        5: EmergencyStop();
        6: MaintenanceMode();
    END_CASE;
    
    // Continuous operations
    DataLogging();
    StatisticsCalculation();
    
    // Increment cycle counter
    cycle_counter := cycle_counter + 1;
    
    ACTIONS
        ACTION SystemInitialization
            // Initialize all systems
            controller.Initialize();
            
            // Clear sensor data
            FOR i := 1 TO 3 DO
                sensor_readings[i].valid := FALSE;
                sensor_readings[i].timestamp := 0;
            END_FOR;
            
            // Reset statistics
            avg_temperature := 0.0;
            max_pressure := 0.0;
            log_index := 1;
            
            // Start initialization timer
            state_timer(IN := TRUE, PT := T#5s);
            
            IF state_timer.Q THEN
                state_timer(IN := FALSE);
                system_state := 1;  // Move to sensor calibration
                LogEvent(1, 'System initialization complete');
            END_IF;
        END_ACTION
        
        ACTION SensorCalibration
            // Read and validate all sensors
            FOR i := 1 TO 3 DO
                current_sensor := &sensor_readings[i];
                current_sensor^.temperature := ReadSensorValue(i * 10 + 1);
                current_sensor^.pressure := ReadSensorValue(i * 10 + 2);
                current_sensor^.flow_rate := ReadSensorValue(i * 10 + 3);
                
                // Validate readings using reference pointers
                temp_ptr := &current_sensor^.temperature;
                pressure_ptr := &current_sensor^.pressure;
                
                IF (temp_ptr^ >= -50.0) AND (temp_ptr^ <= 200.0) AND
                   (pressure_ptr^ >= 0.0) AND (pressure_ptr^ <= 1000.0) THEN
                    current_sensor^.valid := TRUE;
                ELSE
                    current_sensor^.valid := FALSE;
                END_IF;
            END_FOR;
            
            // Check if all sensors are valid
            startup_complete := TRUE;
            FOR i := 1 TO 3 DO
                IF NOT sensor_readings[i].valid THEN
                    startup_complete := FALSE;
                END_IF;
            END_FOR;
            
            IF startup_complete THEN
                system_state := 2;  // Move to process startup
                LogEvent(1, 'Sensor calibration complete');
            END_IF;
        END_ACTION
        
        ACTION ProcessStartup
            // Enable controller
            controller.Enable();
            
            // Set initial setpoints
            controller.SetTemperatureSetpoint(75.0);
            
            // Wait for process to stabilize
            state_timer(IN := TRUE, PT := T#10s);
            
            IF state_timer.Q THEN
                state_timer(IN := FALSE);
                process_running := TRUE;
                system_state := 3;  // Move to normal operation
                LogEvent(1, 'Process startup complete');
            END_IF;
        END_ACTION
        
        ACTION NormalOperation
            // Update controller with sensor data
            controller.UpdateSensorData(sensor_readings[1]);
            
            // Calculate control outputs
            controller.CalculateControl();
            
            // Check for shutdown conditions
            IF shutdown_requested THEN
                system_state := 4;  // Controlled shutdown
            ELSIF emergency_stop THEN
                system_state := 5;  // Emergency stop
            END_IF;
            
            // Continue normal operation
            // (Additional process logic would go here)
        END_ACTION
        
        ACTION ProcessShutdown
            // Controlled shutdown sequence
            controller.Disable();
            
            state_timer(IN := TRUE, PT := T#15s);
            
            IF state_timer.Q THEN
                state_timer(IN := FALSE);
                process_running := FALSE;
                system_state := 0;  // Return to initialization
                LogEvent(1, 'Process shutdown complete');
            END_IF;
        END_ACTION
        
        ACTION EmergencyStop
            // Immediate shutdown
            controller.Disable();
            process_running := FALSE;
            
            LogEvent(3, 'Emergency stop activated');
            
            // Wait for reset
            IF NOT emergency_stop THEN
                system_state := 0;  // Return to initialization
            END_IF;
        END_ACTION
        
        ACTION MaintenanceMode
            // Maintenance operations
            controller.Disable();
            
            // Allow manual control and diagnostics
            // (Maintenance logic would go here)
            
            LogEvent(1, 'Maintenance mode active');
        END_ACTION
        
        ACTION DataLogging
            // Log current sensor data to circular buffer
            IF process_running AND (cycle_counter MOD 10 = 0) THEN  // Log every 10 cycles
                log_ptr := &log_buffer[log_index];
                log_ptr^ := sensor_readings[1];
                
                log_index := log_index + 1;
                IF log_index > 100 THEN
                    log_index := 1;  // Wrap around
                END_IF;
            END_IF;
        END_ACTION
        
        ACTION StatisticsCalculation
            VAR
                temp_sum : REAL := 0.0;
                valid_count : INT := 0;
            END_VAR
            
            // Calculate running statistics using reference operations
            FOR i := 1 TO 3 DO
                current_sensor := &sensor_readings[i];
                
                IF current_sensor^.valid THEN
                    temp_ptr := &current_sensor^.temperature;
                    pressure_ptr := &current_sensor^.pressure;
                    
                    temp_sum := temp_sum + temp_ptr^;
                    valid_count := valid_count + 1;
                    
                    IF pressure_ptr^ > max_pressure THEN
                        max_pressure := pressure_ptr^;
                    END_IF;
                END_IF;
            END_FOR;
            
            IF valid_count > 0 THEN
                avg_temperature := temp_sum / valid_count;
            END_IF;
        END_ACTION
    END_ACTIONS
END_PROGRAM

// Utility functions using reference parameters
FUNCTION ValidateAndClamp
    VAR_INPUT
        {ref} value : REAL;
        min_val : REAL;
        max_val : REAL;
    END_VAR
    VAR_OUTPUT
        clamped : BOOL := FALSE;
    END_VAR
    
    IF value < min_val THEN
        value := min_val;
        clamped := TRUE;
    ELSIF value > max_val THEN
        value := max_val;
        clamped := TRUE;
    END_IF;
END_FUNCTION

FUNCTION ProcessSensorArray : INT
    VAR_INPUT
        {ref} sensors : ARRAY[1..3] OF SensorData;
    END_VAR
    VAR
        i : INT;
        valid_count : INT := 0;
        sensor_ptr : SensorRef;
        temp_ptr : TempRef;
        was_clamped : BOOL;
    END_VAR
    
    FOR i := 1 TO 3 DO
        sensor_ptr := &sensors[i];
        
        IF sensor_ptr^.valid THEN
            // Validate and clamp temperature readings
            temp_ptr := &sensor_ptr^.temperature;
            ValidateAndClamp(temp_ptr^, -40.0, 150.0, was_clamped);
            
            IF was_clamped THEN
                LogEvent(2, 'Sensor reading clamped to valid range');
            END_IF;
            
            valid_count := valid_count + 1;
        END_IF;
    END_FOR;
    
    ProcessSensorArray := valid_count;
END_FUNCTION