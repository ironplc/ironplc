PROGRAM main
    VAR
        Button AT %IX1: BOOL;
        Buzzer AT %QX1: BOOL;
    END_VAR
    VAR_ACCESS
        VarPath : VarName.Path : BOOL READ_ONLY;
    END_VAR

    Buzzer := NOT Button;

END_PROGRAM