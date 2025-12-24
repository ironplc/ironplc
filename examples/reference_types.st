(*
    Example demonstrating reference types and pointer operations
    for indirect access and dynamic data structures.
*)

TYPE
    IntRef : REF_TO INT;
    RealRef : REF_TO REAL;
    IntRefRef : REF_TO REF_TO INT;
    
    // Structure with reference member
    DataNode : STRUCT
        value : INT;
        next : REF_TO DataNode;
    END_STRUCT;
    
    NodeRef : REF_TO DataNode;
END_TYPE

PROGRAM ReferenceTypesDemo
    VAR
        // Basic variables
        number : INT := 42;
        decimal : REAL := 3.14;
        
        // Reference variables
        int_ptr : IntRef;
        real_ptr : RealRef;
        double_ptr : IntRefRef;
        
        // Variables for testing
        result : INT;
        temp_value : REAL;
        
        // Array for pointer arithmetic simulation
        numbers : ARRAY[1..5] OF INT := [10, 20, 30, 40, 50];
        array_ptr : IntRef;
        
        // Linked list nodes
        node1, node2, node3 : DataNode;
        list_head : NodeRef;
        current_node : NodeRef;
        list_sum : INT := 0;
    END_VAR
    
    // Basic pointer operations
    int_ptr := &number;        // Take address of number
    result := int_ptr^;        // Dereference to get value (42)
    int_ptr^ := 100;           // Modify through pointer (number becomes 100)
    
    // Real number pointer
    real_ptr := &decimal;
    temp_value := real_ptr^;   // Get value through pointer
    real_ptr^ := 2.71;         // Modify through pointer
    
    // Double indirection
    double_ptr := &int_ptr;    // Pointer to pointer
    result := double_ptr^^;    // Double dereference
    double_ptr^^ := 200;       // Modify through double pointer
    
    // Null pointer handling
    int_ptr := NULL;
    IF int_ptr <> NULL THEN
        result := int_ptr^;    // Safe dereference
    END_IF;
    
    // Array access through pointers (simulated)
    array_ptr := &numbers[3];  // Point to third element
    result := array_ptr^;      // Should get 30
    array_ptr^ := 35;          // Modify array element
    
    // Linked list operations
    // Initialize nodes
    node1.value := 10;
    node2.value := 20;
    node3.value := 30;
    
    // Link nodes together
    node1.next := &node2;
    node2.next := &node3;
    node3.next := NULL;
    
    // Traverse linked list
    list_head := &node1;
    current_node := list_head;
    
    WHILE current_node <> NULL DO
        list_sum := list_sum + current_node^.value;
        current_node := current_node^.next;
    END_WHILE;
    // list_sum should be 60 (10 + 20 + 30)
END_PROGRAM

// Advanced reference operations example
PROGRAM AdvancedReferenceDemo
    VAR
        // Matrix represented as array of arrays
        matrix : ARRAY[1..3] OF ARRAY[1..3] OF INT;
        row_ptr : REF_TO ARRAY[1..3] OF INT;
        element_ptr : IntRef;
        
        // Function pointer simulation using references
        operation_result : INT;
        
        // Dynamic memory simulation
        buffer : ARRAY[1..100] OF INT;
        buffer_ptr : IntRef;
        buffer_index : INT := 1;
        
        i, j : INT;
    END_VAR
    
    // Initialize matrix
    FOR i := 1 TO 3 DO
        FOR j := 1 TO 3 DO
            matrix[i][j] := i * 10 + j;
        END_FOR;
    END_FOR;
    
    // Access matrix through row pointer
    row_ptr := &matrix[2];     // Point to second row
    operation_result := row_ptr^[2];  // Get element [2,2] = 22
    
    // Modify matrix through pointer
    element_ptr := &matrix[1][1];
    element_ptr^ := 99;        // matrix[1][1] becomes 99
    
    // Buffer operations with pointer arithmetic simulation
    buffer_ptr := &buffer[buffer_index];
    
    FOR i := 1 TO 10 DO
        buffer_ptr^ := i * i;  // Store square numbers
        buffer_index := buffer_index + 1;
        IF buffer_index <= 100 THEN
            buffer_ptr := &buffer[buffer_index];
        END_IF;
    END_FOR;
    
    // Verify buffer contents
    operation_result := buffer[5];  // Should be 25 (5^2)
END_PROGRAM

// Reference parameter with pointer operations
FUNCTION ProcessArray : INT
    VAR_INPUT
        {ref} arr : ARRAY[1..10] OF INT;
        size : INT;
    END_VAR
    VAR
        ptr : IntRef;
        i : INT;
        sum : INT := 0;
    END_VAR
    
    // Process array using pointer-style access
    FOR i := 1 TO size DO
        ptr := &arr[i];
        sum := sum + ptr^;
        ptr^ := ptr^ * 2;  // Double each element
    END_FOR;
    
    ProcessArray := sum;
END_FUNCTION

PROGRAM ReferenceParameterPointerDemo
    VAR
        data : ARRAY[1..10] OF INT := [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        original_sum : INT;
        final_sum : INT;
    END_VAR
    
    // Process array - function will double all elements and return original sum
    original_sum := ProcessArray(data, 10);  // Returns 55
    
    // Calculate sum of doubled values
    final_sum := ProcessArray(data, 10);     // Returns 110 (doubled values)
    // Note: data array now contains [4, 8, 12, 16, 20, 24, 28, 32, 36, 40]
END_PROGRAM