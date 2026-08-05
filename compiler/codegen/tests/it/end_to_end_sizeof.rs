//! End-to-end integration tests for the SIZEOF operator.

use ironplc_parser::options::CompilerOptions;

fn sizeof_options() -> CompilerOptions {
    CompilerOptions {
        allow_sizeof: true,
        ..CompilerOptions::default()
    }
}

e2e_i32_with!(
    end_to_end_when_sizeof_int_then_returns_2,
    sizeof_options(),
    "
PROGRAM main
  VAR
    x : INT;
    s : DINT;
  END_VAR
  s := SIZEOF(x);
END_PROGRAM
",
    &[(1, 2)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_dint_then_returns_4,
    sizeof_options(),
    "
PROGRAM main
  VAR
    x : DINT;
    s : DINT;
  END_VAR
  s := SIZEOF(x);
END_PROGRAM
",
    &[(1, 4)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_dword_then_returns_4,
    sizeof_options(),
    "
PROGRAM main
  VAR
    y : DWORD;
    s : DINT;
  END_VAR
  s := SIZEOF(y);
END_PROGRAM
",
    &[(1, 4)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_bool_then_returns_1,
    sizeof_options(),
    "
PROGRAM main
  VAR
    b : BOOL;
    s : DINT;
  END_VAR
  s := SIZEOF(b);
END_PROGRAM
",
    &[(1, 1)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_real_then_returns_4,
    sizeof_options(),
    "
PROGRAM main
  VAR
    r : REAL;
    s : DINT;
  END_VAR
  s := SIZEOF(r);
END_PROGRAM
",
    &[(1, 4)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_lreal_then_returns_8,
    sizeof_options(),
    "
PROGRAM main
  VAR
    r : LREAL;
    s : DINT;
  END_VAR
  s := SIZEOF(r);
END_PROGRAM
",
    &[(1, 8)],
);

// 10 elements × 2 bytes each = 20
e2e_i32_with!(
    end_to_end_when_sizeof_array_of_int_then_returns_total_bytes,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[1..10] OF INT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[(1, 20)],
);
