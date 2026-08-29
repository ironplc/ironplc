//! Built-in function IDs used with the BUILTIN opcode.

/// EXPT for 32-bit integers: pops exponent (b) and base (a), pushes a ** b.
/// Traps on negative exponent.
pub const EXPT_I32: u16 = 0x0340;

/// EXPT for 32-bit floats: pops exponent (b) and base (a), pushes a.powf(b).
pub const EXPT_F32: u16 = 0x0341;

/// EXPT for 64-bit floats: pops exponent (b) and base (a), pushes a.powf(b).
pub const EXPT_F64: u16 = 0x0342;

/// ABS for 32-bit integers: pops one value, pushes its absolute value (wrapping).
pub const ABS_I32: u16 = 0x0343;

/// MIN for 32-bit integers: pops two values (b then a), pushes min(a, b).
pub const MIN_I32: u16 = 0x0344;

/// MAX for 32-bit integers: pops two values (b then a), pushes max(a, b).
pub const MAX_I32: u16 = 0x0345;

/// LIMIT for 32-bit integers: pops mx, in, mn, pushes clamp(in, mn, mx).
pub const LIMIT_I32: u16 = 0x0346;

/// SEL for 32-bit integers: pops in1, in0, g, pushes in0 if g==0 else in1.
pub const SEL_I32: u16 = 0x0347;

/// SHL for 32-bit: pops shift count (n) and value (a), pushes a << n.
pub const SHL_I32: u16 = 0x0348;

/// SHL for 64-bit: pops shift count (n) and value (a), pushes a << n.
pub const SHL_I64: u16 = 0x0349;

/// SHR for 32-bit: pops shift count (n) and value (a), pushes a >> n (logical).
pub const SHR_I32: u16 = 0x034A;

/// SHR for 64-bit: pops shift count (n) and value (a), pushes a >> n (logical).
pub const SHR_I64: u16 = 0x034B;

/// ROL for 32-bit: pops shift count (n) and value (a), pushes a.rotate_left(n).
pub const ROL_I32: u16 = 0x034C;

/// ROL for 64-bit: pops shift count (n) and value (a), pushes a.rotate_left(n).
pub const ROL_I64: u16 = 0x034D;

/// ROR for 32-bit: pops shift count (n) and value (a), pushes a.rotate_right(n).
pub const ROR_I32: u16 = 0x034E;

/// ROR for 64-bit: pops shift count (n) and value (a), pushes a.rotate_right(n).
pub const ROR_I64: u16 = 0x034F;

/// ROL for 8-bit (BYTE): narrow rotate within 8 bits.
pub const ROL_U8: u16 = 0x0350;

/// ROL for 16-bit (WORD): narrow rotate within 16 bits.
pub const ROL_U16: u16 = 0x0351;

/// ROR for 8-bit (BYTE): narrow rotate within 8 bits.
pub const ROR_U8: u16 = 0x0352;

/// ROR for 16-bit (WORD): narrow rotate within 16 bits.
pub const ROR_U16: u16 = 0x0353;

/// ABS for 32-bit floats: pops one value, pushes its absolute value.
pub const ABS_F32: u16 = 0x0354;

/// ABS for 64-bit floats: pops one value, pushes its absolute value.
pub const ABS_F64: u16 = 0x0355;

/// MIN for 32-bit floats: pops two values (b then a), pushes min(a, b).
pub const MIN_F32: u16 = 0x0356;

/// MIN for 64-bit floats: pops two values (b then a), pushes min(a, b).
pub const MIN_F64: u16 = 0x0357;

/// MAX for 32-bit floats: pops two values (b then a), pushes max(a, b).
pub const MAX_F32: u16 = 0x0358;

/// MAX for 64-bit floats: pops two values (b then a), pushes max(a, b).
pub const MAX_F64: u16 = 0x0359;

/// LIMIT for 32-bit floats: pops mx, in, mn, pushes clamp(in, mn, mx).
pub const LIMIT_F32: u16 = 0x035A;

/// LIMIT for 64-bit floats: pops mx, in, mn, pushes clamp(in, mn, mx).
pub const LIMIT_F64: u16 = 0x035B;

/// SEL for 32-bit floats: pops in1, in0 (f32), g (i32), pushes in0 if g==0 else in1.
pub const SEL_F32: u16 = 0x035C;

/// SEL for 64-bit floats: pops in1, in0 (f64), g (i32), pushes in0 if g==0 else in1.
pub const SEL_F64: u16 = 0x035D;

/// SQRT for 32-bit floats: pops one value, pushes its square root.
pub const SQRT_F32: u16 = 0x035E;

/// SQRT for 64-bit floats: pops one value, pushes its square root.
pub const SQRT_F64: u16 = 0x035F;

/// EXPT for 64-bit integers: pops exponent (b) and base (a), pushes a ** b.
/// Traps on negative exponent.
pub const EXPT_I64: u16 = 0x0360;

/// ABS for 64-bit integers: pops one value, pushes its absolute value (wrapping).
pub const ABS_I64: u16 = 0x0361;

/// MIN for 64-bit signed integers: pops two values (b then a), pushes min(a, b).
pub const MIN_I64: u16 = 0x0362;

/// MAX for 64-bit signed integers: pops two values (b then a), pushes max(a, b).
pub const MAX_I64: u16 = 0x0363;

/// LIMIT for 64-bit signed integers: pops mx, in, mn, pushes clamp(in, mn, mx).
pub const LIMIT_I64: u16 = 0x0364;

/// SEL for 64-bit values: pops in1, in0 (i64), g (i32), pushes in0 if g==0 else in1.
pub const SEL_I64: u16 = 0x0365;

/// MIN for 32-bit unsigned integers: pops two values (b then a), pushes unsigned min.
pub const MIN_U32: u16 = 0x0366;

/// MAX for 32-bit unsigned integers: pops two values (b then a), pushes unsigned max.
pub const MAX_U32: u16 = 0x0367;

/// LIMIT for 32-bit unsigned integers: pops mx, in, mn, pushes unsigned clamp.
pub const LIMIT_U32: u16 = 0x0368;

/// MIN for 64-bit unsigned integers: pops two values (b then a), pushes unsigned min.
pub const MIN_U64: u16 = 0x0369;

/// MAX for 64-bit unsigned integers: pops two values (b then a), pushes unsigned max.
pub const MAX_U64: u16 = 0x036A;

/// LIMIT for 64-bit unsigned integers: pops mx, in, mn, pushes unsigned clamp.
pub const LIMIT_U64: u16 = 0x036B;

/// LN for 32-bit floats: pops one value, pushes its natural logarithm.
pub const LN_F32: u16 = 0x036C;

/// LN for 64-bit floats: pops one value, pushes its natural logarithm.
pub const LN_F64: u16 = 0x036D;

/// LOG for 32-bit floats: pops one value, pushes its base-10 logarithm.
pub const LOG_F32: u16 = 0x036E;

/// LOG for 64-bit floats: pops one value, pushes its base-10 logarithm.
pub const LOG_F64: u16 = 0x036F;

/// EXP for 32-bit floats: pops one value, pushes e raised to that power.
pub const EXP_F32: u16 = 0x0370;

/// EXP for 64-bit floats: pops one value, pushes e raised to that power.
pub const EXP_F64: u16 = 0x0371;

/// SIN for 32-bit floats: pops one value (radians), pushes its sine.
pub const SIN_F32: u16 = 0x0372;

/// SIN for 64-bit floats: pops one value (radians), pushes its sine.
pub const SIN_F64: u16 = 0x0373;

/// COS for 32-bit floats: pops one value (radians), pushes its cosine.
pub const COS_F32: u16 = 0x0374;

/// COS for 64-bit floats: pops one value (radians), pushes its cosine.
pub const COS_F64: u16 = 0x0375;

/// TAN for 32-bit floats: pops one value (radians), pushes its tangent.
pub const TAN_F32: u16 = 0x0376;

/// TAN for 64-bit floats: pops one value (radians), pushes its tangent.
pub const TAN_F64: u16 = 0x0377;

/// ASIN for 32-bit floats: pops one value, pushes its arc sine (radians).
pub const ASIN_F32: u16 = 0x0378;

/// ASIN for 64-bit floats: pops one value, pushes its arc sine (radians).
pub const ASIN_F64: u16 = 0x0379;

/// ACOS for 32-bit floats: pops one value, pushes its arc cosine (radians).
pub const ACOS_F32: u16 = 0x037A;

/// ACOS for 64-bit floats: pops one value, pushes its arc cosine (radians).
pub const ACOS_F64: u16 = 0x037B;

/// ATAN for 32-bit floats: pops one value, pushes its arc tangent (radians).
pub const ATAN_F32: u16 = 0x037C;

/// ATAN for 64-bit floats: pops one value, pushes its arc tangent (radians).
pub const ATAN_F64: u16 = 0x037D;

// --- Type conversion opcodes ---

/// Convert signed 32-bit integer to 32-bit float.
pub const CONV_I32_TO_F32: u16 = 0x037E;

/// Convert signed 32-bit integer to 64-bit float.
pub const CONV_I32_TO_F64: u16 = 0x037F;

/// Convert signed 64-bit integer to 32-bit float.
pub const CONV_I64_TO_F32: u16 = 0x0380;

/// Convert signed 64-bit integer to 64-bit float.
pub const CONV_I64_TO_F64: u16 = 0x0381;

/// Convert unsigned 32-bit integer to 32-bit float.
pub const CONV_U32_TO_F32: u16 = 0x0382;

/// Convert unsigned 32-bit integer to 64-bit float.
pub const CONV_U32_TO_F64: u16 = 0x0383;

/// Convert unsigned 64-bit integer to 32-bit float.
pub const CONV_U64_TO_F32: u16 = 0x0384;

/// Convert unsigned 64-bit integer to 64-bit float.
pub const CONV_U64_TO_F64: u16 = 0x0385;

/// Convert 32-bit float to signed 32-bit integer (truncating).
pub const CONV_F32_TO_I32: u16 = 0x0386;

/// Convert 32-bit float to signed 64-bit integer (truncating).
pub const CONV_F32_TO_I64: u16 = 0x0387;

/// Convert 64-bit float to signed 32-bit integer (truncating).
pub const CONV_F64_TO_I32: u16 = 0x0388;

/// Convert 64-bit float to signed 64-bit integer (truncating).
pub const CONV_F64_TO_I64: u16 = 0x0389;

/// Convert 32-bit float to unsigned 32-bit integer (truncating).
pub const CONV_F32_TO_U32: u16 = 0x038A;

/// Convert 32-bit float to unsigned 64-bit integer (truncating).
pub const CONV_F32_TO_U64: u16 = 0x038B;

/// Convert 64-bit float to unsigned 32-bit integer (truncating).
pub const CONV_F64_TO_U32: u16 = 0x038C;

/// Convert 64-bit float to unsigned 64-bit integer (truncating).
pub const CONV_F64_TO_U64: u16 = 0x038D;

/// Widen 32-bit float to 64-bit float.
pub const CONV_F32_TO_F64: u16 = 0x038E;

/// Narrow 64-bit float to 32-bit float.
pub const CONV_F64_TO_F32: u16 = 0x038F;

/// Zero-extend unsigned 32-bit integer to 64-bit integer.
pub const CONV_U32_TO_I64: u16 = 0x0390;

// --- BCD conversion opcodes ---

/// BCD_TO_INT for 8-bit (BYTE → USINT): decode 2 BCD digits.
pub const BCD_TO_INT_8: u16 = 0x0391;

/// BCD_TO_INT for 16-bit (WORD → UINT): decode 4 BCD digits.
pub const BCD_TO_INT_16: u16 = 0x0392;

/// BCD_TO_INT for 32-bit (DWORD → UDINT): decode 8 BCD digits.
pub const BCD_TO_INT_32: u16 = 0x0393;

/// BCD_TO_INT for 64-bit (LWORD → ULINT): decode 16 BCD digits.
pub const BCD_TO_INT_64: u16 = 0x0394;

/// INT_TO_BCD for 8-bit (USINT → BYTE): encode 2 BCD digits.
pub const INT_TO_BCD_8: u16 = 0x0395;

/// INT_TO_BCD for 16-bit (UINT → WORD): encode 4 BCD digits.
pub const INT_TO_BCD_16: u16 = 0x0396;

/// INT_TO_BCD for 32-bit (UDINT → DWORD): encode 8 BCD digits.
pub const INT_TO_BCD_32: u16 = 0x0397;

/// INT_TO_BCD for 64-bit (ULINT → LWORD): encode 16 BCD digits.
pub const INT_TO_BCD_64: u16 = 0x0398;

// --- Integer to boolean conversion opcodes ---

/// Convert 32-bit integer to boolean: 0 → FALSE (0), non-zero → TRUE (1).
pub const CONV_I32_TO_BOOL: u16 = 0x0399;

/// Convert 64-bit integer to boolean: 0 → FALSE (0), non-zero → TRUE (1).
pub const CONV_I64_TO_BOOL: u16 = 0x039A;

// --- Two-argument trigonometric opcodes ---

/// ATAN2 for 32-bit floats: pops two values (b=IN2=X, a=IN1=Y), pushes atan2(Y, X).
pub const ATAN2_F32: u16 = 0x039B;

/// ATAN2 for 64-bit floats: pops two values (b=IN2=X, a=IN1=Y), pushes atan2(Y, X).
pub const ATAN2_F64: u16 = 0x039C;

// =========================================================================
// Numeric ↔ STRING conversion builtins
//
// These are dispatched inline in the VM main loop (not via
// builtin::dispatch) because they need access to temp buffers and
// the data region.
// =========================================================================

/// Convert signed 32-bit integer to decimal string.
/// Stack: pop i32, push buf_idx (temp buffer with result).
pub const CONV_I32_TO_STR: u16 = 0x039D;

/// Convert unsigned 32-bit integer to decimal string.
/// Stack: pop i32 (treated as u32), push buf_idx.
pub const CONV_U32_TO_STR: u16 = 0x039E;

/// Parse decimal string to signed 32-bit integer.
/// Stack: pop data_offset (i32), push parsed i32 (0 on failure).
pub const CONV_STR_TO_I32: u16 = 0x039F;

/// Convert 32-bit float to decimal string.
/// Stack: pop f32, push buf_idx (temp buffer with result).
pub const CONV_F32_TO_STR: u16 = 0x03A0;

/// Parse decimal string to 32-bit float.
/// Stack: pop data_offset (i32), push parsed f32 (0.0 on failure).
pub const CONV_STR_TO_F32: u16 = 0x03A1;

/// Three-way lexicographic string comparison.
/// Pops right_data_offset (i32) then left_data_offset (i32).
/// Pushes -1 (left < right), 0 (equal), or +1 (left > right) as i32.
pub const CMP_STR: u16 = 0x03A2;

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
pub const TRUNC_F64: u16 = 0x03A3;

/// Floating-point modulo with the sign of the dividend (Rust `%` on f64,
/// i.e. fmod; `x % 0.0` is NaN, not a trap): pops divisor then dividend,
/// pushes the remainder.
pub const MOD_F64: u16 = 0x03A4;

/// REAL-preserving truncation toward zero (`f32::trunc`): the f32 variant
/// of [`TRUNC_F64`].
pub const TRUNC_F32: u16 = 0x03A5;

/// Floating-point modulo with the sign of the dividend on f32: the f32
/// variant of [`MOD_F64`] (`x % 0.0` is NaN, not a trap).
pub const MOD_F32: u16 = 0x03A6;

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
    Some(match func_id {
        ABS_I32 | ABS_F32 | ABS_F64 | ABS_I64 | SQRT_F32 | SQRT_F64 | LN_F32 | LN_F64 | LOG_F32
        | LOG_F64 | EXP_F32 | EXP_F64 | SIN_F32 | SIN_F64 | COS_F32 | COS_F64 | TAN_F32
        | TAN_F64 | ASIN_F32 | ASIN_F64 | ACOS_F32 | ACOS_F64 | ATAN_F32 | ATAN_F64
        | CONV_I32_TO_F32 | CONV_I32_TO_F64 | CONV_I64_TO_F32 | CONV_I64_TO_F64
        | CONV_U32_TO_F32 | CONV_U32_TO_F64 | CONV_U64_TO_F32 | CONV_U64_TO_F64
        | CONV_F32_TO_I32 | CONV_F32_TO_I64 | CONV_F64_TO_I32 | CONV_F64_TO_I64
        | CONV_F32_TO_U32 | CONV_F32_TO_U64 | CONV_F64_TO_U32 | CONV_F64_TO_U64
        | CONV_F32_TO_F64 | CONV_F64_TO_F32 | CONV_U32_TO_I64 | BCD_TO_INT_8 | BCD_TO_INT_16
        | BCD_TO_INT_32 | BCD_TO_INT_64 | INT_TO_BCD_8 | INT_TO_BCD_16 | INT_TO_BCD_32
        | INT_TO_BCD_64 | CONV_I32_TO_BOOL | CONV_I64_TO_BOOL | CONV_I32_TO_STR
        | CONV_U32_TO_STR | CONV_STR_TO_I32 | CONV_F32_TO_STR | CONV_STR_TO_F32 | TRUNC_F64
        | TRUNC_F32 => 1,
        EXPT_I32 | EXPT_F32 | EXPT_F64 | EXPT_I64 | MIN_I32 | MIN_F32 | MIN_F64 | MIN_I64
        | MIN_U32 | MIN_U64 | MAX_I32 | MAX_F32 | MAX_F64 | MAX_I64 | MAX_U32 | MAX_U64
        | SHL_I32 | SHL_I64 | SHR_I32 | SHR_I64 | ROL_I32 | ROL_I64 | ROR_I32 | ROR_I64
        | ROL_U8 | ROL_U16 | ROR_U8 | ROR_U16 | ATAN2_F32 | ATAN2_F64 | CMP_STR | MOD_F64
        | MOD_F32 => 2,
        LIMIT_I32 | LIMIT_F32 | LIMIT_F64 | LIMIT_I64 | LIMIT_U32 | LIMIT_U64 | SEL_I32
        | SEL_F32 | SEL_F64 | SEL_I64 => 3,
        id if is_mux(id) => {
            // MUX pops n IN values + 1 K selector
            mux_info(id)? + 1
        }
        _ => return None,
    })
}
