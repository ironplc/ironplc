(*
    Example demonstrating action blocks for organizing
    code into reusable named sections.
*)

PROGRAM StateMachineDemo
    VAR
        state : INT := 0;
        timer : TON;
        counter : INT := 0;
        process_complete : BOOL := FALSE;
        error_occurred : BOOL := FALSE;
        reset_requested : BOOL := FALSE;
    END_VAR
    
    // Main state machine logic
    CASE state OF
        0: Initialize();      // Initialization state
        1: WaitForStart();    // Wait for start condition
        2: ProcessData();     // Main processing
        3: Finalize();        // Cleanup and finalization
        4: ErrorHandling();   // Error recovery
    END_CASE;
    
    // Reset logic
    IF reset_requested THEN
        ResetSystem();
    END_IF;
    
    ACTIONS
        ACTION Initialize
            // System initialization
            counter := 0;
            process_complete := FALSE;
            error_occurred := FALSE;
            timer(IN := FALSE);  // Reset timer
            
            // Move to next state
            state := 1;
        END_ACTION
        
        ACTION WaitForStart
            // Wait for start condition (could be external input)
            timer(IN := TRUE, PT := T#2s);
            
            IF timer.Q THEN
                timer(IN := FALSE);
                state := 2;  // Move to processing
            END_IF;
        END_ACTION
        
        ACTION ProcessData
            // Main processing logic
            counter := counter + 1;
            
            // Simulate processing time
            timer(IN := TRUE, PT := T#5s);
            
            IF counter >= 100 THEN
                // Processing complete
                process_complete := TRUE;
                state := 3;
            ELSIF timer.Q THEN
                // Check for error condition
                IF counter MOD 10 = 0 THEN
                    error_occurred := TRUE;
                    state := 4;
                ELSE
                    timer(IN := FALSE);  // Reset for next cycle
                END_IF;
            END_IF;
        END_ACTION
        
        ACTION Finalize
            // Cleanup and finalization
            timer(IN := TRUE, PT := T#1s);
            
            IF timer.Q THEN
                // Return to initialization for next cycle
                state := 0;
                timer(IN := FALSE);
            END_IF;
        END_ACTION
        
        ACTION ErrorHandling
            // Error recovery procedures
            timer(IN := TRUE, PT := T#3s);
            
            IF timer.Q THEN
                // Clear error and retry
                error_occurred := FALSE;
                counter := counter - 5;  // Backtrack slightly
                state := 2;  // Return to processing
                timer(IN := FALSE);
            END_IF;
        END_ACTION
        
        ACTION ResetSystem
            // Emergency reset action
            state := 0;
            counter := 0;
            process_complete := FALSE;
            error_occurred := FALSE;
            reset_requested := FALSE;
            timer(IN := FALSE);
        END_ACTION
    END_ACTIONS
END_PROGRAM

// Example of action blocks for modular operations
PROGRAM ModularOperationsDemo
    VAR
        operation_mode : INT := 1;
        input_data : ARRAY[1..10] OF INT := [5, 2, 8, 1, 9, 3, 7, 4, 6, 0];
        output_data : ARRAY[1..10] OF INT;
        processing_done : BOOL := FALSE;
    END_VAR
    
    CASE operation_mode OF
        1: SortAscending();
        2: SortDescending();
        3: FindMinMax();
        4: CalculateSum();
    END_CASE;
    
    ACTIONS
        ACTION SortAscending
            VAR
                i, j, temp : INT;
            END_VAR
            
            // Copy input to output
            FOR i := 1 TO 10 DO
                output_data[i] := input_data[i];
            END_FOR;
            
            // Bubble sort ascending
            FOR i := 1 TO 9 DO
                FOR j := 1 TO 10 - i DO
                    IF output_data[j] > output_data[j + 1] THEN
                        temp := output_data[j];
                        output_data[j] := output_data[j + 1];
                        output_data[j + 1] := temp;
                    END_IF;
                END_FOR;
            END_FOR;
            
            processing_done := TRUE;
        END_ACTION
        
        ACTION SortDescending
            VAR
                i, j, temp : INT;
            END_VAR
            
            // Copy input to output
            FOR i := 1 TO 10 DO
                output_data[i] := input_data[i];
            END_FOR;
            
            // Bubble sort descending
            FOR i := 1 TO 9 DO
                FOR j := 1 TO 10 - i DO
                    IF output_data[j] < output_data[j + 1] THEN
                        temp := output_data[j];
                        output_data[j] := output_data[j + 1];
                        output_data[j + 1] := temp;
                    END_IF;
                END_FOR;
            END_FOR;
            
            processing_done := TRUE;
        END_ACTION
        
        ACTION FindMinMax
            VAR
                i : INT;
                min_val, max_val : INT;
            END_VAR
            
            min_val := input_data[1];
            max_val := input_data[1];
            
            FOR i := 2 TO 10 DO
                IF input_data[i] < min_val THEN
                    min_val := input_data[i];
                END_IF;
                IF input_data[i] > max_val THEN
                    max_val := input_data[i];
                END_IF;
            END_FOR;
            
            output_data[1] := min_val;
            output_data[2] := max_val;
            processing_done := TRUE;
        END_ACTION
        
        ACTION CalculateSum
            VAR
                i, sum : INT;
            END_VAR
            
            sum := 0;
            FOR i := 1 TO 10 DO
                sum := sum + input_data[i];
            END_FOR;
            
            output_data[1] := sum;
            processing_done := TRUE;
        END_ACTION
    END_ACTIONS
END_PROGRAM