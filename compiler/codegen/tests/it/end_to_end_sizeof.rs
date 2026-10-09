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
    &[("s", 2)],
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
    &[("s", 4)],
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
    &[("s", 4)],
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
    &[("s", 1)],
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
    &[("s", 4)],
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
    &[("s", 8)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_int_then_returns_8,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[0..3] OF INT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 8)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_sint_then_returns_element_count,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[1..10] OF SINT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 10)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_dint_then_returns_4_per_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[0..4] OF DINT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 20)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_lint_then_returns_8_per_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[1..3] OF LINT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 24)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_real_then_returns_4_per_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[0..2] OF REAL;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 12)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_lreal_then_returns_8_per_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[0..1] OF LREAL;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 16)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_byte_then_returns_element_count,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[0..7] OF BYTE;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 8)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_word_then_returns_2_per_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[0..3] OF WORD;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 8)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_bool_then_returns_1_per_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[1..8] OF BOOL;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 8)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_with_negative_lower_bound_then_counts_every_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[-5..5] OF INT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 22)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_with_nonzero_lower_bound_then_counts_every_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[10..19] OF INT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 20)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_of_one_element_then_returns_element_size,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[5..5] OF DINT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 4)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_two_dimensional_array_then_returns_product_of_dimensions,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[1..3, 1..4] OF INT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 24)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_three_dimensional_array_then_returns_product_of_dimensions,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[0..1, 0..2, 0..3] OF DINT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 96)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_multi_dimensional_array_with_negative_bounds_then_counts_every_element,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[-1..1, 2..3] OF LINT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr);
END_PROGRAM
",
    &[("s", 48)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_array_element_then_returns_element_size,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[0..3] OF DINT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr[2]);
END_PROGRAM
",
    &[("s", 4)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_multi_dimensional_array_element_then_returns_element_size,
    sizeof_options(),
    "
PROGRAM main
  VAR
    arr : ARRAY[1..3, 1..4] OF LINT;
    s : DINT;
  END_VAR
  s := SIZEOF(arr[2, 3]);
END_PROGRAM
",
    &[("s", 8)],
);

e2e_i32_with!(
    end_to_end_when_sizeof_named_multi_dimensional_array_then_returns_product_of_dimensions,
    sizeof_options(),
    "
TYPE
  MATRIX : ARRAY[1..2, 1..5] OF REAL;
END_TYPE

PROGRAM main
  VAR
    m : MATRIX;
    s : DINT;
  END_VAR
  s := SIZEOF(m);
END_PROGRAM
",
    &[("s", 40)],
);
