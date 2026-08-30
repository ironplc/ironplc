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
