//! Built-in function IDs used with the BUILTIN opcode.

/// Declares the built-in functions: one row per built-in, giving its name, the
/// function ID that the `BUILTIN` opcode's operand carries, and how many
/// arguments it pops.
///
/// A row is the only declaration of a built-in. The ID constant, the name a
/// disassembler shows, and the argument count codegen and the verifier read
/// all come from it, so a built-in cannot exist while being nameless or
/// unsized -- the states that previously left `BUILTIN` rows rendering as bare
/// hex.
macro_rules! declare_builtins {
    ($(
        $(#[$meta:meta])*
        $name:ident = $id:literal, args $args:literal;
    )*) => {
        $(
            $(#[$meta])*
            pub const $name: u16 = $id;
        )*

        /// The name of the built-in `func_id` calls, or `None` when no
        /// built-in has that ID.
        ///
        /// MUX is not here: its IDs are ranges rather than single values, so
        /// they are recognised by [`mux_info`] and named by [`mux_type_name`].
        pub fn name(func_id: u16) -> Option<&'static str> {
            match func_id {
                $($name => Some(stringify!($name)),)*
                _ => None,
            }
        }

        /// The number of arguments `func_id` pops, for the built-ins whose ID
        /// is a single value. [`arg_count_opt`] adds the MUX ranges.
        fn declared_arg_count(func_id: u16) -> Option<u16> {
            match func_id {
                $($name => Some($args),)*
                _ => None,
            }
        }
    };
}

declare_builtins! {
    /// EXPT for 32-bit integers: pops exponent (b) and base (a), pushes a ** b.
    /// Traps on negative exponent.
    EXPT_I32 = 0x0340, args 2;

    /// EXPT for 32-bit floats: pops exponent (b) and base (a), pushes a.powf(b).
    EXPT_F32 = 0x0341, args 2;

    /// EXPT for 64-bit floats: pops exponent (b) and base (a), pushes a.powf(b).
    EXPT_F64 = 0x0342, args 2;

    /// ABS for 32-bit integers: pops one value, pushes its absolute value (wrapping).
    ABS_I32 = 0x0343, args 1;

    /// MIN for 32-bit integers: pops two values (b then a), pushes min(a, b).
    MIN_I32 = 0x0344, args 2;

    /// MAX for 32-bit integers: pops two values (b then a), pushes max(a, b).
    MAX_I32 = 0x0345, args 2;

    /// LIMIT for 32-bit integers: pops mx, in, mn, pushes clamp(in, mn, mx).
    LIMIT_I32 = 0x0346, args 3;

    /// SEL for 32-bit integers: pops in1, in0, g, pushes in0 if g==0 else in1.
    SEL_I32 = 0x0347, args 3;

    /// SHL for 32-bit: pops shift count (n) and value (a), pushes a << n.
    SHL_I32 = 0x0348, args 2;

    /// SHL for 64-bit: pops shift count (n) and value (a), pushes a << n.
    SHL_I64 = 0x0349, args 2;

    /// SHR for 32-bit: pops shift count (n) and value (a), pushes a >> n (logical).
    SHR_I32 = 0x034A, args 2;

    /// SHR for 64-bit: pops shift count (n) and value (a), pushes a >> n (logical).
    SHR_I64 = 0x034B, args 2;

    /// ROL for 32-bit: pops shift count (n) and value (a), pushes a.rotate_left(n).
    ROL_I32 = 0x034C, args 2;

    /// ROL for 64-bit: pops shift count (n) and value (a), pushes a.rotate_left(n).
    ROL_I64 = 0x034D, args 2;

    /// ROR for 32-bit: pops shift count (n) and value (a), pushes a.rotate_right(n).
    ROR_I32 = 0x034E, args 2;

    /// ROR for 64-bit: pops shift count (n) and value (a), pushes a.rotate_right(n).
    ROR_I64 = 0x034F, args 2;

    /// ROL for 8-bit (BYTE): narrow rotate within 8 bits.
    ROL_U8 = 0x0350, args 2;

    /// ROL for 16-bit (WORD): narrow rotate within 16 bits.
    ROL_U16 = 0x0351, args 2;

    /// ROR for 8-bit (BYTE): narrow rotate within 8 bits.
    ROR_U8 = 0x0352, args 2;

    /// ROR for 16-bit (WORD): narrow rotate within 16 bits.
    ROR_U16 = 0x0353, args 2;

    /// ABS for 32-bit floats: pops one value, pushes its absolute value.
    ABS_F32 = 0x0354, args 1;

    /// ABS for 64-bit floats: pops one value, pushes its absolute value.
    ABS_F64 = 0x0355, args 1;

    /// MIN for 32-bit floats: pops two values (b then a), pushes min(a, b).
    MIN_F32 = 0x0356, args 2;

    /// MIN for 64-bit floats: pops two values (b then a), pushes min(a, b).
    MIN_F64 = 0x0357, args 2;

    /// MAX for 32-bit floats: pops two values (b then a), pushes max(a, b).
    MAX_F32 = 0x0358, args 2;

    /// MAX for 64-bit floats: pops two values (b then a), pushes max(a, b).
    MAX_F64 = 0x0359, args 2;

    /// LIMIT for 32-bit floats: pops mx, in, mn, pushes clamp(in, mn, mx).
    LIMIT_F32 = 0x035A, args 3;

    /// LIMIT for 64-bit floats: pops mx, in, mn, pushes clamp(in, mn, mx).
    LIMIT_F64 = 0x035B, args 3;

    /// SEL for 32-bit floats: pops in1, in0 (f32), g (i32), pushes in0 if g==0 else in1.
    SEL_F32 = 0x035C, args 3;

    /// SEL for 64-bit floats: pops in1, in0 (f64), g (i32), pushes in0 if g==0 else in1.
    SEL_F64 = 0x035D, args 3;

    /// SQRT for 32-bit floats: pops one value, pushes its square root.
    SQRT_F32 = 0x035E, args 1;

    /// SQRT for 64-bit floats: pops one value, pushes its square root.
    SQRT_F64 = 0x035F, args 1;

    /// EXPT for 64-bit integers: pops exponent (b) and base (a), pushes a ** b.
    /// Traps on negative exponent.
    EXPT_I64 = 0x0360, args 2;

    /// ABS for 64-bit integers: pops one value, pushes its absolute value (wrapping).
    ABS_I64 = 0x0361, args 1;

    /// MIN for 64-bit signed integers: pops two values (b then a), pushes min(a, b).
    MIN_I64 = 0x0362, args 2;

    /// MAX for 64-bit signed integers: pops two values (b then a), pushes max(a, b).
    MAX_I64 = 0x0363, args 2;

    /// LIMIT for 64-bit signed integers: pops mx, in, mn, pushes clamp(in, mn, mx).
    LIMIT_I64 = 0x0364, args 3;

    /// SEL for 64-bit values: pops in1, in0 (i64), g (i32), pushes in0 if g==0 else in1.
    SEL_I64 = 0x0365, args 3;

    /// MIN for 32-bit unsigned integers: pops two values (b then a), pushes unsigned min.
    MIN_U32 = 0x0366, args 2;

    /// MAX for 32-bit unsigned integers: pops two values (b then a), pushes unsigned max.
    MAX_U32 = 0x0367, args 2;

    /// LIMIT for 32-bit unsigned integers: pops mx, in, mn, pushes unsigned clamp.
    LIMIT_U32 = 0x0368, args 3;

    /// MIN for 64-bit unsigned integers: pops two values (b then a), pushes unsigned min.
    MIN_U64 = 0x0369, args 2;

    /// MAX for 64-bit unsigned integers: pops two values (b then a), pushes unsigned max.
    MAX_U64 = 0x036A, args 2;

    /// LIMIT for 64-bit unsigned integers: pops mx, in, mn, pushes unsigned clamp.
    LIMIT_U64 = 0x036B, args 3;

    /// LN for 32-bit floats: pops one value, pushes its natural logarithm.
    LN_F32 = 0x036C, args 1;

    /// LN for 64-bit floats: pops one value, pushes its natural logarithm.
    LN_F64 = 0x036D, args 1;

    /// LOG for 32-bit floats: pops one value, pushes its base-10 logarithm.
    LOG_F32 = 0x036E, args 1;

    /// LOG for 64-bit floats: pops one value, pushes its base-10 logarithm.
    LOG_F64 = 0x036F, args 1;

    /// EXP for 32-bit floats: pops one value, pushes e raised to that power.
    EXP_F32 = 0x0370, args 1;

    /// EXP for 64-bit floats: pops one value, pushes e raised to that power.
    EXP_F64 = 0x0371, args 1;

    /// SIN for 32-bit floats: pops one value (radians), pushes its sine.
    SIN_F32 = 0x0372, args 1;

    /// SIN for 64-bit floats: pops one value (radians), pushes its sine.
    SIN_F64 = 0x0373, args 1;

    /// COS for 32-bit floats: pops one value (radians), pushes its cosine.
    COS_F32 = 0x0374, args 1;

    /// COS for 64-bit floats: pops one value (radians), pushes its cosine.
    COS_F64 = 0x0375, args 1;

    /// TAN for 32-bit floats: pops one value (radians), pushes its tangent.
    TAN_F32 = 0x0376, args 1;

    /// TAN for 64-bit floats: pops one value (radians), pushes its tangent.
    TAN_F64 = 0x0377, args 1;

    /// ASIN for 32-bit floats: pops one value, pushes its arc sine (radians).
    ASIN_F32 = 0x0378, args 1;

    /// ASIN for 64-bit floats: pops one value, pushes its arc sine (radians).
    ASIN_F64 = 0x0379, args 1;

    /// ACOS for 32-bit floats: pops one value, pushes its arc cosine (radians).
    ACOS_F32 = 0x037A, args 1;

    /// ACOS for 64-bit floats: pops one value, pushes its arc cosine (radians).
    ACOS_F64 = 0x037B, args 1;

    /// ATAN for 32-bit floats: pops one value, pushes its arc tangent (radians).
    ATAN_F32 = 0x037C, args 1;

    /// ATAN for 64-bit floats: pops one value, pushes its arc tangent (radians).
    ATAN_F64 = 0x037D, args 1;

    // --- Type conversion opcodes ---

    /// Convert signed 32-bit integer to 32-bit float.
    CONV_I32_TO_F32 = 0x037E, args 1;

    /// Convert signed 32-bit integer to 64-bit float.
    CONV_I32_TO_F64 = 0x037F, args 1;

    /// Convert signed 64-bit integer to 32-bit float.
    CONV_I64_TO_F32 = 0x0380, args 1;

    /// Convert signed 64-bit integer to 64-bit float.
    CONV_I64_TO_F64 = 0x0381, args 1;

    /// Convert unsigned 32-bit integer to 32-bit float.
    CONV_U32_TO_F32 = 0x0382, args 1;

    /// Convert unsigned 32-bit integer to 64-bit float.
    CONV_U32_TO_F64 = 0x0383, args 1;

    /// Convert unsigned 64-bit integer to 32-bit float.
    CONV_U64_TO_F32 = 0x0384, args 1;

    /// Convert unsigned 64-bit integer to 64-bit float.
    CONV_U64_TO_F64 = 0x0385, args 1;

    /// Convert 32-bit float to signed 32-bit integer (truncating).
    CONV_F32_TO_I32 = 0x0386, args 1;

    /// Convert 32-bit float to signed 64-bit integer (truncating).
    CONV_F32_TO_I64 = 0x0387, args 1;

    /// Convert 64-bit float to signed 32-bit integer (truncating).
    CONV_F64_TO_I32 = 0x0388, args 1;

    /// Convert 64-bit float to signed 64-bit integer (truncating).
    CONV_F64_TO_I64 = 0x0389, args 1;

    /// Convert 32-bit float to unsigned 32-bit integer (truncating).
    CONV_F32_TO_U32 = 0x038A, args 1;

    /// Convert 32-bit float to unsigned 64-bit integer (truncating).
    CONV_F32_TO_U64 = 0x038B, args 1;

    /// Convert 64-bit float to unsigned 32-bit integer (truncating).
    CONV_F64_TO_U32 = 0x038C, args 1;

    /// Convert 64-bit float to unsigned 64-bit integer (truncating).
    CONV_F64_TO_U64 = 0x038D, args 1;

    /// Widen 32-bit float to 64-bit float.
    CONV_F32_TO_F64 = 0x038E, args 1;

    /// Narrow 64-bit float to 32-bit float.
    CONV_F64_TO_F32 = 0x038F, args 1;

    /// Zero-extend unsigned 32-bit integer to 64-bit integer.
    CONV_U32_TO_I64 = 0x0390, args 1;

    // --- BCD conversion opcodes ---

    /// BCD_TO_INT for 8-bit (BYTE → USINT): decode 2 BCD digits.
    BCD_TO_INT_8 = 0x0391, args 1;

    /// BCD_TO_INT for 16-bit (WORD → UINT): decode 4 BCD digits.
    BCD_TO_INT_16 = 0x0392, args 1;

    /// BCD_TO_INT for 32-bit (DWORD → UDINT): decode 8 BCD digits.
    BCD_TO_INT_32 = 0x0393, args 1;

    /// BCD_TO_INT for 64-bit (LWORD → ULINT): decode 16 BCD digits.
    BCD_TO_INT_64 = 0x0394, args 1;

    /// INT_TO_BCD for 8-bit (USINT → BYTE): encode 2 BCD digits.
    INT_TO_BCD_8 = 0x0395, args 1;

    /// INT_TO_BCD for 16-bit (UINT → WORD): encode 4 BCD digits.
    INT_TO_BCD_16 = 0x0396, args 1;

    /// INT_TO_BCD for 32-bit (UDINT → DWORD): encode 8 BCD digits.
    INT_TO_BCD_32 = 0x0397, args 1;

    /// INT_TO_BCD for 64-bit (ULINT → LWORD): encode 16 BCD digits.
    INT_TO_BCD_64 = 0x0398, args 1;

    // --- Integer to boolean conversion opcodes ---

    /// Convert 32-bit integer to boolean: 0 → FALSE (0), non-zero → TRUE (1).
    CONV_I32_TO_BOOL = 0x0399, args 1;

    /// Convert 64-bit integer to boolean: 0 → FALSE (0), non-zero → TRUE (1).
    CONV_I64_TO_BOOL = 0x039A, args 1;

    // --- Two-argument trigonometric opcodes ---

    /// ATAN2 for 32-bit floats: pops two values (b=IN2=X, a=IN1=Y), pushes atan2(Y, X).
    ATAN2_F32 = 0x039B, args 2;

    /// ATAN2 for 64-bit floats: pops two values (b=IN2=X, a=IN1=Y), pushes atan2(Y, X).
    ATAN2_F64 = 0x039C, args 2;

    // =========================================================================
    // Numeric ↔ STRING conversion builtins
    //
    // These are dispatched inline in the VM main loop (not via
    // builtin::dispatch) because they need access to temp buffers and
    // the data region.
    // =========================================================================

    /// Convert signed 32-bit integer to decimal string.
    /// Stack: pop i32, push buf_idx (temp buffer with result).
    CONV_I32_TO_STR = 0x039D, args 1;

    /// Convert unsigned 32-bit integer to decimal string.
    /// Stack: pop i32 (treated as u32), push buf_idx.
    CONV_U32_TO_STR = 0x039E, args 1;

    /// Parse decimal string to signed 32-bit integer.
    /// Stack: pop data_offset (i32), push parsed i32 (0 on failure).
    CONV_STR_TO_I32 = 0x039F, args 1;

    /// Convert 32-bit float to decimal string.
    /// Stack: pop f32, push buf_idx (temp buffer with result).
    CONV_F32_TO_STR = 0x03A0, args 1;

    /// Parse decimal string to 32-bit float.
    /// Stack: pop data_offset (i32), push parsed f32 (0.0 on failure).
    CONV_STR_TO_F32 = 0x03A1, args 1;

    /// Three-way lexicographic string comparison.
    /// Pops right_data_offset (i32) then left_data_offset (i32).
    /// Pushes -1 (left < right), 0 (equal), or +1 (left > right) as i32.
    CMP_STR = 0x03A2, args 2;

    // =========================================================================
    // Real truncation / floating-modulo builtins
    //
    // These implement real-number semantics that IEC 61131-3 source cannot
    // express (ADR-0042): truncation that stays in the real type, and a
    // floating modulo. They are the lowering targets of the `__TRUNC` /
    // `__MOD` compiler intrinsics (ANY_REAL, width selects the variant).
    // =========================================================================

    /// LREAL-preserving truncation toward zero (`f64::trunc`): pops one f64,
    /// pushes its integer part as f64. The result stays f64, so values beyond
    /// any integer range are preserved exactly rather than clamped.
    TRUNC_F64 = 0x03A3, args 1;

    /// Floating-point modulo with the sign of the dividend (Rust `%` on f64,
    /// i.e. fmod; `x % 0.0` is NaN, not a trap): pops divisor then dividend,
    /// pushes the remainder.
    MOD_F64 = 0x03A4, args 2;

    /// REAL-preserving truncation toward zero (`f32::trunc`): the f32 variant
    /// of [`TRUNC_F64`].
    TRUNC_F32 = 0x03A5, args 1;

    /// Floating-point modulo with the sign of the dividend on f32: the f32
    /// variant of [`MOD_F64`] (`x % 0.0` is NaN, not a trap).
    MOD_F32 = 0x03A6, args 2;

    // =========================================================================
    // STRING_TO_<numeric> under behavior policies (ADR-0049)
    //
    // One row per (target, non-numeric alternative, failure alternative), laid
    // out in the block `str_to_num` describes so the VM can decode the two
    // policies from the ID. Dispatched inline in the VM main loop like the
    // other string conversions. Stack: pop data_offset (i32), push the value.
    // =========================================================================

    /// Parse a STRING to an unsigned 32-bit integer: the whole string must be
    /// a literal (`reject`); a failure traps `V4006` (`trap`).
    CONV_STR_TO_U32_REJECT_TRAP = 0x0480, args 1;

    /// As [`CONV_STR_TO_U32_REJECT_TRAP`], but a failure yields 0 (`zero`).
    CONV_STR_TO_U32_REJECT_ZERO = 0x0481, args 1;

    /// Parse a STRING to an unsigned 32-bit integer: the leading literal is
    /// converted and the rest ignored (`ignore-trailing`); a failure traps.
    CONV_STR_TO_U32_IGNORE_TRAILING_TRAP = 0x0482, args 1;

    /// As [`CONV_STR_TO_U32_IGNORE_TRAILING_TRAP`], but a failure yields 0.
    CONV_STR_TO_U32_IGNORE_TRAILING_ZERO = 0x0483, args 1;

    /// Parse a STRING to an unsigned 32-bit integer: leading non-literal
    /// characters are skipped, then the literal is converted and the rest
    /// ignored (`ignore-surrounding`); a failure traps.
    CONV_STR_TO_U32_IGNORE_SURROUNDING_TRAP = 0x0484, args 1;

    /// As [`CONV_STR_TO_U32_IGNORE_SURROUNDING_TRAP`], but a failure yields 0.
    CONV_STR_TO_U32_IGNORE_SURROUNDING_ZERO = 0x0485, args 1;

    /// Parse a STRING to a signed 32-bit integer (`STRING_TO_DINT`): the six
    /// rows of target 1, laid out as the `CONV_STR_TO_U32_*` rows are.
    CONV_STR_TO_I32_REJECT_TRAP = 0x0488, args 1;
    /// As [`CONV_STR_TO_I32_REJECT_TRAP`], but a failure yields 0.
    CONV_STR_TO_I32_REJECT_ZERO = 0x0489, args 1;
    /// As [`CONV_STR_TO_I32_REJECT_TRAP`] under `ignore-trailing`.
    CONV_STR_TO_I32_IGNORE_TRAILING_TRAP = 0x048A, args 1;
    /// As [`CONV_STR_TO_I32_IGNORE_TRAILING_TRAP`], but a failure yields 0.
    CONV_STR_TO_I32_IGNORE_TRAILING_ZERO = 0x048B, args 1;
    /// As [`CONV_STR_TO_I32_REJECT_TRAP`] under `ignore-surrounding`.
    CONV_STR_TO_I32_IGNORE_SURROUNDING_TRAP = 0x048C, args 1;
    /// As [`CONV_STR_TO_I32_IGNORE_SURROUNDING_TRAP`], but a failure yields 0.
    CONV_STR_TO_I32_IGNORE_SURROUNDING_ZERO = 0x048D, args 1;

    /// Parse a STRING to an unsigned 8-bit integer (`STRING_TO_USINT`,
    /// `STRING_TO_BYTE`): the six rows of target 2, laid out as the
    /// `CONV_STR_TO_U32_*` rows are.
    CONV_STR_TO_U8_REJECT_TRAP = 0x0490, args 1;
    /// As [`CONV_STR_TO_U8_REJECT_TRAP`], but a failure yields 0.
    CONV_STR_TO_U8_REJECT_ZERO = 0x0491, args 1;
    /// As [`CONV_STR_TO_U8_REJECT_TRAP`] under `ignore-trailing`.
    CONV_STR_TO_U8_IGNORE_TRAILING_TRAP = 0x0492, args 1;
    /// As [`CONV_STR_TO_U8_IGNORE_TRAILING_TRAP`], but a failure yields 0.
    CONV_STR_TO_U8_IGNORE_TRAILING_ZERO = 0x0493, args 1;
    /// As [`CONV_STR_TO_U8_REJECT_TRAP`] under `ignore-surrounding`.
    CONV_STR_TO_U8_IGNORE_SURROUNDING_TRAP = 0x0494, args 1;
    /// As [`CONV_STR_TO_U8_IGNORE_SURROUNDING_TRAP`], but a failure yields 0.
    CONV_STR_TO_U8_IGNORE_SURROUNDING_ZERO = 0x0495, args 1;

    /// Parse a STRING to a signed 8-bit integer (`STRING_TO_SINT`): the six rows
    /// of target 3, laid out as the `CONV_STR_TO_U32_*` rows are.
    CONV_STR_TO_I8_REJECT_TRAP = 0x0498, args 1;
    /// As [`CONV_STR_TO_I8_REJECT_TRAP`], but a failure yields 0.
    CONV_STR_TO_I8_REJECT_ZERO = 0x0499, args 1;
    /// As [`CONV_STR_TO_I8_REJECT_TRAP`] under `ignore-trailing`.
    CONV_STR_TO_I8_IGNORE_TRAILING_TRAP = 0x049A, args 1;
    /// As [`CONV_STR_TO_I8_IGNORE_TRAILING_TRAP`], but a failure yields 0.
    CONV_STR_TO_I8_IGNORE_TRAILING_ZERO = 0x049B, args 1;
    /// As [`CONV_STR_TO_I8_REJECT_TRAP`] under `ignore-surrounding`.
    CONV_STR_TO_I8_IGNORE_SURROUNDING_TRAP = 0x049C, args 1;
    /// As [`CONV_STR_TO_I8_IGNORE_SURROUNDING_TRAP`], but a failure yields 0.
    CONV_STR_TO_I8_IGNORE_SURROUNDING_ZERO = 0x049D, args 1;

    /// Parse a STRING to an unsigned 16-bit integer (`STRING_TO_UINT`,
    /// `STRING_TO_WORD`): the six rows of target 4, laid out as the
    /// `CONV_STR_TO_U32_*` rows are.
    CONV_STR_TO_U16_REJECT_TRAP = 0x04A0, args 1;
    /// As [`CONV_STR_TO_U16_REJECT_TRAP`], but a failure yields 0.
    CONV_STR_TO_U16_REJECT_ZERO = 0x04A1, args 1;
    /// As [`CONV_STR_TO_U16_REJECT_TRAP`] under `ignore-trailing`.
    CONV_STR_TO_U16_IGNORE_TRAILING_TRAP = 0x04A2, args 1;
    /// As [`CONV_STR_TO_U16_IGNORE_TRAILING_TRAP`], but a failure yields 0.
    CONV_STR_TO_U16_IGNORE_TRAILING_ZERO = 0x04A3, args 1;
    /// As [`CONV_STR_TO_U16_REJECT_TRAP`] under `ignore-surrounding`.
    CONV_STR_TO_U16_IGNORE_SURROUNDING_TRAP = 0x04A4, args 1;
    /// As [`CONV_STR_TO_U16_IGNORE_SURROUNDING_TRAP`], but a failure yields 0.
    CONV_STR_TO_U16_IGNORE_SURROUNDING_ZERO = 0x04A5, args 1;

    /// Parse a STRING to a signed 16-bit integer (`STRING_TO_INT`): the six rows
    /// of target 5, laid out as the `CONV_STR_TO_U32_*` rows are.
    CONV_STR_TO_I16_REJECT_TRAP = 0x04A8, args 1;
    /// As [`CONV_STR_TO_I16_REJECT_TRAP`], but a failure yields 0.
    CONV_STR_TO_I16_REJECT_ZERO = 0x04A9, args 1;
    /// As [`CONV_STR_TO_I16_REJECT_TRAP`] under `ignore-trailing`.
    CONV_STR_TO_I16_IGNORE_TRAILING_TRAP = 0x04AA, args 1;
    /// As [`CONV_STR_TO_I16_IGNORE_TRAILING_TRAP`], but a failure yields 0.
    CONV_STR_TO_I16_IGNORE_TRAILING_ZERO = 0x04AB, args 1;
    /// As [`CONV_STR_TO_I16_REJECT_TRAP`] under `ignore-surrounding`.
    CONV_STR_TO_I16_IGNORE_SURROUNDING_TRAP = 0x04AC, args 1;
    /// As [`CONV_STR_TO_I16_IGNORE_SURROUNDING_TRAP`], but a failure yields 0.
    CONV_STR_TO_I16_IGNORE_SURROUNDING_ZERO = 0x04AD, args 1;

    /// Parse a STRING to an unsigned 64-bit integer (`STRING_TO_ULINT`,
    /// `STRING_TO_LWORD`): the six rows of target 6, laid out as the
    /// `CONV_STR_TO_U32_*` rows are.
    CONV_STR_TO_U64_REJECT_TRAP = 0x04B0, args 1;
    /// As [`CONV_STR_TO_U64_REJECT_TRAP`], but a failure yields 0.
    CONV_STR_TO_U64_REJECT_ZERO = 0x04B1, args 1;
    /// As [`CONV_STR_TO_U64_REJECT_TRAP`] under `ignore-trailing`.
    CONV_STR_TO_U64_IGNORE_TRAILING_TRAP = 0x04B2, args 1;
    /// As [`CONV_STR_TO_U64_IGNORE_TRAILING_TRAP`], but a failure yields 0.
    CONV_STR_TO_U64_IGNORE_TRAILING_ZERO = 0x04B3, args 1;
    /// As [`CONV_STR_TO_U64_REJECT_TRAP`] under `ignore-surrounding`.
    CONV_STR_TO_U64_IGNORE_SURROUNDING_TRAP = 0x04B4, args 1;
    /// As [`CONV_STR_TO_U64_IGNORE_SURROUNDING_TRAP`], but a failure yields 0.
    CONV_STR_TO_U64_IGNORE_SURROUNDING_ZERO = 0x04B5, args 1;

    /// Parse a STRING to a signed 64-bit integer (`STRING_TO_LINT`): the six
    /// rows of target 7, laid out as the `CONV_STR_TO_U32_*` rows are.
    CONV_STR_TO_I64_REJECT_TRAP = 0x04B8, args 1;
    /// As [`CONV_STR_TO_I64_REJECT_TRAP`], but a failure yields 0.
    CONV_STR_TO_I64_REJECT_ZERO = 0x04B9, args 1;
    /// As [`CONV_STR_TO_I64_REJECT_TRAP`] under `ignore-trailing`.
    CONV_STR_TO_I64_IGNORE_TRAILING_TRAP = 0x04BA, args 1;
    /// As [`CONV_STR_TO_I64_IGNORE_TRAILING_TRAP`], but a failure yields 0.
    CONV_STR_TO_I64_IGNORE_TRAILING_ZERO = 0x04BB, args 1;
    /// As [`CONV_STR_TO_I64_REJECT_TRAP`] under `ignore-surrounding`.
    CONV_STR_TO_I64_IGNORE_SURROUNDING_TRAP = 0x04BC, args 1;
    /// As [`CONV_STR_TO_I64_IGNORE_SURROUNDING_TRAP`], but a failure yields 0.
    CONV_STR_TO_I64_IGNORE_SURROUNDING_ZERO = 0x04BD, args 1;
}

/// The `STRING_TO_<numeric>` func_id block (ADR-0049).
///
/// A conversion's func_id names its target type and both of its policies:
///
/// ```text
/// func_id = BASE + target * TARGET_STRIDE + non_numeric * 2 + failure
/// ```
///
/// Each target owns a stride of eight IDs: three non-numeric alternatives
/// times two failure alternatives, with two spare for a further failure
/// alternative. The block runs to [`END`], room for sixteen targets. Only the
/// IDs whose target is assigned decode; the rest are unknown builtins and
/// trap `V9007` like any other unassigned ID.
///
/// The named rows in [`declare_builtins!`] are the pinned encoding; the
/// arithmetic here must agree with them, and a test checks that it does.
pub mod str_to_num {
    use crate::policy::{BehaviorPolicy, StringToNumFailure, StringToNumNonNumeric};

    /// First func_id of the block.
    pub const BASE: u16 = 0x0480;
    /// Last func_id of the block (inclusive).
    pub const END: u16 = 0x04FF;
    /// func_ids per target type.
    pub const TARGET_STRIDE: u16 = 8;

    /// The numeric type a `STRING_TO_<numeric>` conversion produces.
    ///
    /// The discriminant is the target's position in the block. Even
    /// positions are unsigned and the odd position after each is the signed
    /// type of the same width, in width order 32, 8, 16, 64 (32 first
    /// because `U32` landed at 0); the real targets follow. A bit-string
    /// type (`BYTE`, `WORD`, `DWORD`, `LWORD`) converts as the unsigned
    /// integer of its width and has no target of its own. Positions 8 and 9
    /// are reserved for the real targets; only the targets listed here are
    /// encoded, and `STRING_TO_REAL` still uses the single-encoding builtin
    /// above.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(u16)]
    pub enum Target {
        /// `STRING_TO_UDINT` and `STRING_TO_DWORD` (an unsigned 32-bit integer).
        U32 = 0,
        /// `STRING_TO_DINT` (a signed 32-bit integer).
        I32 = 1,
        /// `STRING_TO_USINT` and `STRING_TO_BYTE` (an unsigned 8-bit integer).
        U8 = 2,
        /// `STRING_TO_SINT` (a signed 8-bit integer).
        I8 = 3,
        /// `STRING_TO_UINT` and `STRING_TO_WORD` (an unsigned 16-bit integer).
        U16 = 4,
        /// `STRING_TO_INT` (a signed 16-bit integer).
        I16 = 5,
        /// `STRING_TO_ULINT` and `STRING_TO_LWORD` (an unsigned 64-bit integer).
        U64 = 6,
        /// `STRING_TO_LINT` (a signed 64-bit integer).
        I64 = 7,
    }

    impl Target {
        /// Every encoded target, in block order.
        pub const ALL: &'static [Target] = &[
            Target::U32,
            Target::I32,
            Target::U8,
            Target::I8,
            Target::U16,
            Target::I16,
            Target::U64,
            Target::I64,
        ];

        fn from_index(index: u16) -> Option<Target> {
            Self::ALL.get(index as usize).copied()
        }
    }

    /// The two policies a decoded `STRING_TO_<numeric>` func_id carries.
    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct Encoding {
        pub target: Target,
        pub non_numeric: StringToNumNonNumeric,
        pub failure: StringToNumFailure,
    }

    /// The func_id for a conversion to `target` under the two policies.
    pub fn func_id(
        target: Target,
        non_numeric: StringToNumNonNumeric,
        failure: StringToNumFailure,
    ) -> u16 {
        BASE + (target as u16) * TARGET_STRIDE + non_numeric.index() * 2 + failure.index()
    }

    /// Decodes a func_id in the block back to its target and policies, or
    /// `None` when `func_id` is outside the block or names no assigned
    /// conversion.
    pub fn decode(func_id: u16) -> Option<Encoding> {
        if !(BASE..=END).contains(&func_id) {
            return None;
        }
        let offset = func_id - BASE;
        let target = Target::from_index(offset / TARGET_STRIDE)?;
        let within = offset % TARGET_STRIDE;
        let non_numeric = StringToNumNonNumeric::from_index(within / 2)?;
        let failure = StringToNumFailure::from_index(within % 2)?;
        Some(Encoding {
            target,
            non_numeric,
            failure,
        })
    }
}

// =========================================================================
// MUX (multiplexer) range-based opcodes
//
// MUX is extensible: the number of IN arguments varies per call site.
// The func_id encodes the arity: BASE + n, where n is the number of
// IN arguments (2..16). Total stack args = n + 1 (n IN values + K selector).
// =========================================================================

/// Base opcode for MUX with 32-bit signed integer values.
/// MUX_I32_BASE + n = MUX with n IN arguments (n = 2..16).
pub const MUX_I32_BASE: u16 = 0x0400;

/// Base opcode for MUX with 64-bit signed integer values.
pub const MUX_I64_BASE: u16 = 0x0420;

/// Base opcode for MUX with 32-bit float values.
pub const MUX_F32_BASE: u16 = 0x0440;

/// Base opcode for MUX with 64-bit float values.
pub const MUX_F64_BASE: u16 = 0x0460;

/// Maximum number of IN arguments for MUX.
pub const MUX_MAX_INPUTS: u16 = 16;

/// Returns true if the given func_id is a MUX opcode.
pub fn is_mux(func_id: u16) -> bool {
    mux_info(func_id).is_some()
}

/// Names the value type a MUX opcode selects between (`"I32"`, `"I64"`,
/// `"F32"` or `"F64"`), or `None` if `func_id` is not a MUX opcode.
///
/// MUX has no single ID to put in the table -- the arity is encoded in the ID
/// -- so a caller that renders built-in names pairs this with [`mux_info`]
/// instead of [`name`].
pub fn mux_type_name(func_id: u16) -> Option<&'static str> {
    mux_info(func_id)?;
    Some(if func_id >= MUX_F64_BASE {
        "F64"
    } else if func_id >= MUX_F32_BASE {
        "F32"
    } else if func_id >= MUX_I64_BASE {
        "I64"
    } else {
        "I32"
    })
}

/// Returns the number of IN arguments for a MUX opcode, or None if not a MUX opcode.
pub fn mux_info(func_id: u16) -> Option<u16> {
    let bases = [MUX_I32_BASE, MUX_I64_BASE, MUX_F32_BASE, MUX_F64_BASE];
    for base in bases {
        if func_id >= base && func_id < base + MUX_MAX_INPUTS + 1 {
            let n = func_id - base;
            if n >= 2 {
                return Some(n);
            }
        }
    }
    None
}

/// Returns the number of arguments a built-in function pops from the stack.
///
/// This is the single source of truth for argument counts, used by both
/// the codegen emitter (for stack depth tracking) and can be validated
/// against the VM dispatch implementation.
///
/// Panics if `func_id` is not a known built-in function ID. Callers
/// that must not panic on malformed input use [`arg_count_opt`].
pub fn arg_count(func_id: u16) -> u16 {
    arg_count_opt(func_id)
        .unwrap_or_else(|| panic!("unknown builtin function ID: 0x{:04X}", func_id))
}

/// Returns the number of arguments `func_id` pops, or `None` when it is
/// not a known built-in function ID.
///
/// The non-panicking form of [`arg_count`], used by the bytecode
/// verifier, which must report a malformed operand rather than abort.
pub fn arg_count_opt(func_id: u16) -> Option<u16> {
    // MUX pops n IN values + 1 K selector, and its IDs are a range per type
    // rather than one value, so they are not rows in the table.
    declared_arg_count(func_id).or_else(|| Some(mux_info(func_id)? + 1))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::{BehaviorPolicy, StringToNumFailure, StringToNumNonNumeric};
    use std::format;

    #[test]
    fn str_to_num_func_id_when_each_u32_combination_then_matches_declared_row() {
        // The block arithmetic and the pinned rows are two statements of one
        // encoding; this is the check that they agree.
        use str_to_num::{func_id, Target};
        use StringToNumFailure::{Trap, Zero};
        use StringToNumNonNumeric::{IgnoreSurrounding, IgnoreTrailing, Reject};
        assert_eq!(
            func_id(Target::U32, Reject, Trap),
            CONV_STR_TO_U32_REJECT_TRAP
        );
        assert_eq!(
            func_id(Target::U32, Reject, Zero),
            CONV_STR_TO_U32_REJECT_ZERO
        );
        assert_eq!(
            func_id(Target::U32, IgnoreTrailing, Trap),
            CONV_STR_TO_U32_IGNORE_TRAILING_TRAP
        );
        assert_eq!(
            func_id(Target::U32, IgnoreTrailing, Zero),
            CONV_STR_TO_U32_IGNORE_TRAILING_ZERO
        );
        assert_eq!(
            func_id(Target::U32, IgnoreSurrounding, Trap),
            CONV_STR_TO_U32_IGNORE_SURROUNDING_TRAP
        );
        assert_eq!(
            func_id(Target::U32, IgnoreSurrounding, Zero),
            CONV_STR_TO_U32_IGNORE_SURROUNDING_ZERO
        );
    }

    #[test]
    fn str_to_num_func_id_when_each_target_then_first_row_is_at_its_stride() {
        // Each target's six rows start at BASE + position * 8; the rows within
        // a stride follow the U32 layout the test above pins.
        use str_to_num::{func_id, Target};
        use StringToNumFailure::Trap;
        use StringToNumNonNumeric::Reject;
        assert_eq!(
            func_id(Target::I32, Reject, Trap),
            CONV_STR_TO_I32_REJECT_TRAP
        );
        assert_eq!(
            func_id(Target::U8, Reject, Trap),
            CONV_STR_TO_U8_REJECT_TRAP
        );
        assert_eq!(
            func_id(Target::I8, Reject, Trap),
            CONV_STR_TO_I8_REJECT_TRAP
        );
        assert_eq!(
            func_id(Target::U16, Reject, Trap),
            CONV_STR_TO_U16_REJECT_TRAP
        );
        assert_eq!(
            func_id(Target::I16, Reject, Trap),
            CONV_STR_TO_I16_REJECT_TRAP
        );
        assert_eq!(
            func_id(Target::U64, Reject, Trap),
            CONV_STR_TO_U64_REJECT_TRAP
        );
        assert_eq!(
            func_id(Target::I64, Reject, Trap),
            CONV_STR_TO_I64_REJECT_TRAP
        );
    }

    #[test]
    fn str_to_num_name_when_each_target_row_then_names_target_and_both_policies() {
        // The disassembler shows the alternative: every row's name carries
        // the target, the non-numeric alternative and the failure alternative.
        for target in str_to_num::Target::ALL {
            for non_numeric in StringToNumNonNumeric::ALL {
                for failure in StringToNumFailure::ALL {
                    let id = str_to_num::func_id(*target, *non_numeric, *failure);
                    let row = name(id).unwrap();
                    let expected = format!(
                        "CONV_STR_TO_{target:?}_{}_{}",
                        non_numeric.cli_name().replace('-', "_").to_uppercase(),
                        failure.cli_name().to_uppercase()
                    );
                    assert_eq!(row, expected);
                }
            }
        }
    }

    #[test]
    fn str_to_num_decode_when_each_encoded_id_then_round_trips() {
        for target in str_to_num::Target::ALL {
            for non_numeric in StringToNumNonNumeric::ALL {
                for failure in StringToNumFailure::ALL {
                    let id = str_to_num::func_id(*target, *non_numeric, *failure);
                    let decoded = str_to_num::decode(id).unwrap();
                    assert_eq!(decoded.target, *target);
                    assert_eq!(decoded.non_numeric, *non_numeric);
                    assert_eq!(decoded.failure, *failure);
                }
            }
        }
    }

    #[test]
    fn str_to_num_decode_when_id_is_encoded_then_it_is_a_declared_row_and_vice_versa() {
        // Every ID the block decodes is a named row, and every named row in
        // the block decodes: neither the table nor the arithmetic can claim an
        // ID the other does not.
        for func_id in str_to_num::BASE..=str_to_num::END {
            assert_eq!(
                str_to_num::decode(func_id).is_some(),
                name(func_id).is_some(),
                "builtin 0x{func_id:04X}"
            );
        }
    }

    #[test]
    fn str_to_num_decode_when_outside_block_or_spare_slot_then_none() {
        assert_eq!(str_to_num::decode(str_to_num::BASE - 1), None);
        assert_eq!(str_to_num::decode(str_to_num::END + 1), None);
        // The two spare slots of the U32 stride.
        assert_eq!(str_to_num::decode(0x0486), None);
        assert_eq!(str_to_num::decode(0x0487), None);
        // A reserved, not yet assigned target (position 8, the REAL target).
        assert_eq!(str_to_num::decode(0x04C0), None);
    }

    #[test]
    fn name_when_declared_builtin_then_returns_its_name() {
        assert_eq!(name(EXPT_I32), Some("EXPT_I32"));
        // Named by virtue of being in the table, not by a second list: this
        // one had no name in the container viewer before the table existed.
        assert_eq!(name(CONV_F32_TO_F64), Some("CONV_F32_TO_F64"));
    }

    #[test]
    fn name_when_id_is_not_a_builtin_then_returns_none() {
        assert_eq!(name(0x00FF), None);
    }

    #[test]
    fn name_when_mux_id_then_returns_none_because_mux_is_a_range() {
        // MUX encodes its arity in the ID, so it has no single row to name.
        assert_eq!(name(MUX_I32_BASE + 3), None);
        assert_eq!(mux_type_name(MUX_I32_BASE + 3), Some("I32"));
    }

    #[test]
    fn name_when_builtin_declared_then_arg_count_is_declared_too() {
        // Both come from the same row, so no built-in can be nameless or
        // unsized -- the two states that let a BUILTIN operand render as
        // bare hex or panic the emitter.
        for func_id in 0..=u16::MAX {
            assert_eq!(
                name(func_id).is_some(),
                declared_arg_count(func_id).is_some(),
                "builtin 0x{func_id:04X}"
            );
        }
    }

    #[test]
    fn mux_type_name_when_each_base_then_names_its_type() {
        assert_eq!(mux_type_name(MUX_I32_BASE + 2), Some("I32"));
        assert_eq!(mux_type_name(MUX_I64_BASE + 2), Some("I64"));
        assert_eq!(mux_type_name(MUX_F32_BASE + 2), Some("F32"));
        assert_eq!(mux_type_name(MUX_F64_BASE + 2), Some("F64"));
    }

    #[test]
    fn mux_type_name_when_not_a_mux_id_then_returns_none() {
        assert_eq!(mux_type_name(EXPT_I32), None);
    }

    #[test]
    fn arg_count_opt_when_mux_id_then_counts_inputs_plus_selector() {
        assert_eq!(arg_count_opt(MUX_F64_BASE + 5), Some(6));
    }
}
