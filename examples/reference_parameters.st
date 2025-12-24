(*
    Example demonstrating reference parameters for efficient
    parameter passing and in-place modification.
*)

// Function that swaps two integer values using reference parameters
FUNCTION SwapIntegers
    VAR_INPUT
        {ref} a : INT;
        {ref} b : INT;
    END_VAR
    VAR
        temp : INT;
    END_VAR
    
    temp := a;
    a := b;
    b := temp;
END_FUNCTION

// Function that increments a value by a given amount
FUNCTION IncrementBy
    VAR_INPUT
        {ref} value : INT;
        amount : INT;
    END_VAR
    
    value := value + amount;
END_FUNCTION

// Function that normalizes an array of values
FUNCTION NormalizeArray
    VAR_INPUT
        {ref} values : ARRAY[1..10] OF REAL;
        count : INT;
    END_VAR
    VAR
        i : INT;
        max_val : REAL := 0.0;
    END_VAR
    
    // Find maximum value
    FOR i := 1 TO count DO
        IF values[i] > max_val THEN
            max_val := values[i];
        END_IF;
    END_FOR;
    
    // Normalize all values
    IF max_val > 0.0 THEN
        FOR i := 1 TO count DO
            values[i] := values[i] / max_val;
        END_FOR;
    END_IF;
END_FUNCTION

PROGRAM ReferenceParameterDemo
    VAR
        x : INT := 10;
        y : INT := 20;
        counter : INT := 0;
        data : ARRAY[1..10] OF REAL := [1.0, 2.5, 3.7, 4.2, 5.8, 6.1, 7.3, 8.9, 9.4, 10.0];
    END_VAR
    
    // Demonstrate value swapping
    // Before: x=10, y=20
    SwapIntegers(x, y);
    // After: x=20, y=10
    
    // Demonstrate in-place increment
    IncrementBy(counter, 5);  // counter becomes 5
    IncrementBy(counter, 3);  // counter becomes 8
    
    // Demonstrate array modification
    NormalizeArray(data, 10);  // All values normalized to 0.0-1.0 range
END_PROGRAM