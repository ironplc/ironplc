(*
 * Industrial Automation Example
 * 
 * This example demonstrates practical usage of enhanced IronPLC syntax
 * in a typical industrial automation scenario - a conveyor belt system
 * with multiple stations and safety interlocks.
 *)

(* Define custom types for the automation system *)
TYPE
    (* Station operating modes *)
    StationMode : (OFF, MANUAL, AUTO, MAINTENANCE);
    
    (* Conveyor belt speeds (RPM) *)
    BeltSpeed : INT(0..1800);
    
    (* Safety levels *)
    SafetyLevel : (SAFE, CAUTION, DANGER, EMERGENCY);
    
    (* Product types *)
    ProductType : (NONE, TYPE_A, TYPE_B, TYPE_C);
END_TYPE

(* Global system variables *)
VAR_GLOBAL
    (* System control *)
    system_running : BOOL := FALSE;
    emergency_stop_active : BOOL := FALSE;
    
    (* Station modes - 4 stations *)
    station_modes : ARRAY[1..4] OF StationMode;
    
    (* Conveyor belt speeds *)
    belt_speeds : ARRAY[1..3] OF BeltSpeed;  (* 3 conveyor belts *)
    
    (* Safety system *)
    safety_status : SafetyLevel := SAFE;
    light_curtain_status : ARRAY[1..6] OF BOOL;  (* 6 light curtains *)
    
    (* Production tracking *)
    products_on_belt : ARRAY[1..10] OF ProductType;  (* Track up to 10 products *)
    production_count : ARRAY[1..3] OF INT;  (* Count by product type *)
    
    (* Sensor readings *)
    proximity_sensors : ARRAY[1..8] OF BOOL;
    temperature_sensors : ARRAY[1..4] OF INT;
    
    (* Quality control data *)
    quality_matrix : ARRAY[1..3, 1..5] OF BOOL;  (* 3 products x 5 quality checks *)
END_VAR

(* Additional global variables for maintenance *)
VAR_GLOBAL
    (* Maintenance counters *)
    cycle_counters : ARRAY[1..4] OF INT;
    maintenance_due : ARRAY[1..4] OF BOOL;
    
    (* Performance metrics *)
    efficiency_percentage : INT(0..100) := 85;
    uptime_hours : INT := 0;
END_VAR

(* Main conveyor control program *)
PROGRAM ConveyorControl
VAR
    i : INT;
    current_product : ProductType;
    station_ready : BOOL;
END_VAR

    (* Initialize system if not running *)
    IF NOT system_running AND NOT emergency_stop_active THEN
        (* Set all stations to manual mode initially *)
        FOR i := 1 TO 4 DO
            station_modes[i] := MANUAL;
            cycle_counters[i] := 0;
            maintenance_due[i] := FALSE;
        END_FOR;
        
        (* Initialize belt speeds *)
        belt_speeds[1] := 600;   (* Infeed belt *)
        belt_speeds[2] := 800;   (* Main belt *)
        belt_speeds[3] := 400;   (* Outfeed belt *)
        
        system_running := TRUE;
    END_IF;
    
    (* Safety system monitoring *)
    safety_status := SAFE;
    
    (* Check light curtains *)
    FOR i := 1 TO 6 DO
        IF NOT light_curtain_status[i] THEN
            safety_status := DANGER;
        END_IF;
    END_FOR;
    
    (* Check temperature sensors *)
    FOR i := 1 TO 4 DO
        IF temperature_sensors[i] > 70 THEN
            safety_status := CAUTION;
        ELSIF temperature_sensors[i] > 75 THEN
            safety_status := DANGER;
        END_IF;
    END_FOR;
    
    (* Emergency stop handling *)
    IF emergency_stop_active OR safety_status = EMERGENCY THEN
        (* Stop all belts *)
        FOR i := 1 TO 3 DO
            belt_speeds[i] := 0;
        END_FOR;
        
        (* Set all stations to off *)
        FOR i := 1 TO 4 DO
            station_modes[i] := OFF;
        END_FOR;
        
        system_running := FALSE;
    END_IF;
    
    (* Production control based on safety status *)
    CASE safety_status OF
        SAFE:
            (* Normal operation - all stations can run auto *)
            FOR i := 1 TO 4 DO
                IF station_modes[i] = MANUAL THEN
                    station_modes[i] := AUTO;
                END_IF;
            END_FOR;
            
        CAUTION:
            (* Reduced speed operation *)
            FOR i := 1 TO 3 DO
                belt_speeds[i] := belt_speeds[i] / 2;
            END_FOR;
            
        DANGER:
            (* Manual operation only *)
            FOR i := 1 TO 4 DO
                IF station_modes[i] = AUTO THEN
                    station_modes[i] := MANUAL;
                END_IF;
            END_FOR;
            
        EMERGENCY:
            (* Handled above - system shutdown *)
            emergency_stop_active := TRUE;
    END_CASE;
    
    (* Product tracking and counting *)
    FOR i := 1 TO 10 DO
        current_product := products_on_belt[i];
        
        CASE current_product OF
            TYPE_A:
                (* Check quality for Type A *)
                IF quality_matrix[1, 1] AND quality_matrix[1, 2] THEN
                    production_count[1] := production_count[1] + 1;
                END_IF;
                
            TYPE_B:
                (* Check quality for Type B *)
                IF quality_matrix[2, 1] AND quality_matrix[2, 3] THEN
                    production_count[2] := production_count[2] + 1;
                END_IF;
                
            TYPE_C:
                (* Check quality for Type C *)
                IF quality_matrix[3, 4] AND quality_matrix[3, 5] THEN
                    production_count[3] := production_count[3] + 1;
                END_IF;
                
            NONE:
                (* No product - continue *)
                i := i;
        END_CASE;
    END_FOR;
    
    (* Maintenance scheduling *)
    FOR i := 1 TO 4 DO
        cycle_counters[i] := cycle_counters[i] + 1;
        
        (* Schedule maintenance every 10000 cycles *)
        IF cycle_counters[i] > 10000 THEN
            maintenance_due[i] := TRUE;
            station_modes[i] := MAINTENANCE;
        END_IF;
    END_FOR;
    
    (* Calculate system efficiency *)
    IF system_running THEN
        uptime_hours := uptime_hours + 1;
        
        (* Simple efficiency calculation based on production *)
        IF (production_count[1] + production_count[2] + production_count[3]) > 100 THEN
            efficiency_percentage := 95;
        ELSIF (production_count[1] + production_count[2] + production_count[3]) > 50 THEN
            efficiency_percentage := 80;
        ELSE
            efficiency_percentage := 60;
        END_IF;
    END_IF;

END_PROGRAM

(* Station control function block *)
FUNCTION_BLOCK StationController
VAR_INPUT
    station_id : INT(1..4);
    enable : BOOL;
END_VAR
VAR_OUTPUT
    ready : BOOL;
    fault : BOOL;
END_VAR
VAR
    local_mode : StationMode;
    sensor_check : BOOL;
END_VAR

    (* Get station mode from global array *)
    local_mode := station_modes[station_id];
    
    (* Check sensors for this station *)
    sensor_check := proximity_sensors[station_id] AND proximity_sensors[station_id + 4];
    
    (* Station control logic *)
    CASE local_mode OF
        OFF:
            ready := FALSE;
            fault := FALSE;
            
        MANUAL:
            ready := enable AND sensor_check;
            fault := NOT sensor_check;
            
        AUTO:
            ready := sensor_check AND NOT maintenance_due[station_id];
            fault := NOT sensor_check OR maintenance_due[station_id];
            
        MAINTENANCE:
            ready := FALSE;
            fault := FALSE;
            
            (* Reset maintenance flag when maintenance is complete *)
            IF NOT enable THEN
                maintenance_due[station_id] := FALSE;
                cycle_counters[station_id] := 0;
                station_modes[station_id] := MANUAL;
            END_IF;
    END_CASE;

END_FUNCTION_BLOCK