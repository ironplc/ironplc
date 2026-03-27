# Plan: TIME as 32-bit, LTIME as 64-bit

## Summary

Change the internal representation of TIME from 64-bit to 32-bit (milliseconds), and add LTIME as the 64-bit variant (microseconds). LTIME is an IEC 61131-3 Edition 3 (2013) feature and should only be available with a language flag. The focus is on getting the internal representation right in the analyzer, codegen, VM, and playground layers.

## Design Decisions

- **TIME**: 32-bit signed integer, milliseconds. Max range ~24.8 days.
- **LTIME**: 64-bit signed integer, microseconds. Max range ~292,000 years.
- **IntermediateType::Time** gets a `size: ByteSized` parameter (like `Int`/`UInt`), not a separate variant. This follows the existing pattern for integer types.
- **LTIME token gating** via `CompilerOptions` is deferred — tokens are parsed unconditionally and rejected by a post-tokenization validation rule. This matches the existing C-style comment gating pattern.
- **Timer FBs** (TON, TOF, TP) use TIME (32-bit ms) for their PT/ET fields. The VM intrinsics must switch from i64 to i32 for these fields.

## Implementation Steps

### Step 1: Update `IntermediateType::Time` to carry a size

**File**: `compiler/analyzer/src/intermediate_type.rs`

Change `Time` from a unit variant to a sized variant:
```rust
// Before
Time,
// After
Time { size: ByteSized },
```

Update all methods that match on `Time`:
- `size_in_bytes()`: return `size.as_bytes()` instead of hardcoded `Some(8)`
- `alignment_bytes()`: return `size.as_bytes()` instead of hardcoded `8`
- `is_primitive()`: should still return `true`
- Any `PartialEq` or display impls

### Step 2: Update `ELEMENTARY_TYPES_LOWER_CASE` array

**File**: `compiler/analyzer/src/type_environment.rs`

```rust
// TIME is 32-bit (milliseconds)
("time", IntermediateType::Time { size: ByteSized::B32 }),
// LTIME is 64-bit (microseconds) — Edition 3 (2013) feature
("ltime", IntermediateType::Time { size: ByteSized::B64 }),
// TIME_OF_DAY and TOD remain as Time (32-bit for now, until supported)
("time_of_day", IntermediateType::Time { size: ByteSized::B32 }),
("tod", IntermediateType::Time { size: ByteSized::B32 }),
```

Note: The array size constant will need to increase from 23 to 24 to accommodate LTIME.

### Step 3: Update stdlib timer function blocks

**File**: `compiler/analyzer/src/intermediates/stdlib_function_block.rs`

Change `time_type()` helper:
```rust
fn time_type() -> IntermediateType {
    IntermediateType::Time { size: ByteSized::B32 }
}
```

### Step 4: Update codegen type resolution

**File**: `compiler/codegen/src/compile.rs`

Update `resolve_type_name()` to handle both TIME and LTIME:
```rust
"time" => Some(VarTypeInfo { op_width: OpWidth::W32, signedness: Signedness::Signed, storage_bits: 32 }),
"ltime" => Some(VarTypeInfo { op_width: OpWidth::W64, signedness: Signedness::Signed, storage_bits: 64 }),
```

Update `resolve_iec_type_tag()`:
```rust
"time" => iec_type_tag::TIME,
"ltime" => iec_type_tag::LTIME,  // needs new tag constant
```

Update `fb_field_op_type()` for timer fields:
```rust
"pt" | "et" => (OpWidth::W32, Signedness::Signed),  // was W64
```

Update duration literal compilation to emit `emit_load_const_i32` with milliseconds instead of `emit_load_const_i64` with microseconds. Use `whole_milliseconds()` instead of `whole_microseconds()`. This applies to `ConstantKind::Duration` in the `compile_constant()` function.

### Step 5: Add LTIME debug type tag

**File**: `compiler/container/src/debug_section.rs`

Add a new tag constant:
```rust
pub const LTIME: u8 = 23;  // or next available tag number
```

Verify this doesn't conflict with any existing tag in ADR-0019.

### Step 6: Update VM timer intrinsics

**File**: `compiler/vm/src/intrinsic.rs`

Change timer PT and ET field operations from `read_i64`/`write_i64` to `read_i32`/`write_i32`:
- `ton()`: PT read as i32, ET write as i32, elapsed computation in i32 (milliseconds)
- `tof()`: same changes
- `tp()`: same changes

The hidden `start_time` and `running` fields stay as i64 and i32 respectively (start_time needs i64 because cycle_time from the VM is i64 microseconds). However, the **elapsed calculation** must convert from microseconds (cycle_time) to milliseconds for comparison with PT and assignment to ET:
```rust
let elapsed_ms = (cycle_time - start_time) / 1000;  // convert us to ms
let et = if elapsed_ms > pt { pt } else { elapsed_ms as i32 };
write_i32(instance, TIMER_ET, et);
```

### Step 7: Update playground display

**File**: `compiler/playground/src/lib.rs`

Update `format_variable_value()`:
- `iec_type_tag::TIME` → `format_time_value_ms(raw as i32)` (millisecond display)
- `iec_type_tag::LTIME` → `format_time_value_us(raw as i64)` (current microsecond display)

Rename `format_time_value()` to `format_time_value_us()` and add `format_time_value_ms()` for 32-bit millisecond values.

### Step 8: Update documentation

**File**: `docs/reference/language/data-types/time.rst`
- Change size from "64 bits (microsecond resolution)" to "32 bits (millisecond resolution)"

### Step 9: Add LTIME token and parser support (minimal)

**File**: `compiler/parser/src/token.rs`
- Add `#[token("LTIME", ignore(case))] Ltime,` token variant

**File**: `compiler/dsl/src/common.rs`
- Add `LTIME` variant to `ElementaryTypeName` enum
- Add conversion impls (`From<ElementaryTypeName>` for `Id`, `TypeName`)

**File**: `compiler/parser/src/parser.rs`
- Add `tok(TokenType::Ltime) { ElementaryTypeName::LTIME }` to `elementary_type_name()` rule

### Step 10: Add LTIME token gating (deferred — can be a follow-up)

**File**: `compiler/parser/src/options.rs`
- Add `pub allow_edition_3_types: bool` to `CompilerOptions`

**File**: `compiler/parser/src/` (new file `rule_token_no_edition_3.rs`)
- Create a validation rule that rejects `TokenType::Ltime` (and future Edition 3 tokens) when `allow_edition_3_types` is false
- Follow the pattern in `rule_token_no_c_style_comment.rs`

**File**: `compiler/plc2x/src/cli.rs`
- Add `--edition-3` or `--iec-2013` CLI flag
- Wire it to `CompilerOptions::allow_edition_3_types`

### Step 11: Update existing tests

All existing TIME tests use microsecond values and i64 operations. They need to be updated:

**File**: `compiler/codegen/tests/end_to_end_fb_ton.rs`
- Change `T#5s` expectations from 5_000_000 (microseconds) to 5000 (milliseconds)
- Change `read_variable_i64` calls to `read_variable` (i32)
- Update elapsed time expectations accordingly

**File**: `compiler/playground/src/lib.rs` (tests)
- Update `format_time_value` test expectations for millisecond format
- Update TON stepping test expectations

**File**: `compiler/vm/` (any timer tests)
- Update to use i32 for PT/ET values

### Step 12: Add new tests

- Test that `TIME` variables are 32-bit (4 bytes storage)
- Test that `LTIME` variables are 64-bit (8 bytes storage)
- Test that duration literals for TIME are compiled as i32 milliseconds
- Test that timer FBs work correctly with 32-bit TIME values
- Test TIME overflow behavior (values > ~24.8 days)

## Files Changed (Summary)

| File | Change |
|------|--------|
| `analyzer/src/intermediate_type.rs` | `Time` gets `size: ByteSized` |
| `analyzer/src/type_environment.rs` | TIME → B32, add LTIME → B64 |
| `analyzer/src/intermediates/stdlib_function_block.rs` | `time_type()` → B32 |
| `codegen/src/compile.rs` | TIME → W32/32-bit, duration → ms, fb_field → W32 |
| `container/src/debug_section.rs` | Add `LTIME` tag |
| `vm/src/intrinsic.rs` | Timer PT/ET → i32, elapsed → ms |
| `playground/src/lib.rs` | Display formatting for TIME (ms) vs LTIME (us) |
| `parser/src/token.rs` | Add `Ltime` token |
| `dsl/src/common.rs` | Add `LTIME` to `ElementaryTypeName` |
| `parser/src/parser.rs` | Parse LTIME as elementary type |
| `docs/reference/language/data-types/time.rst` | Update size description |
| Various test files | Update expectations for 32-bit ms |

## Risks and Considerations

1. **Breaking change**: Any saved bytecode (.iplc files) with TIME variables will be incompatible. This is acceptable for a pre-1.0 project.
2. **Timer precision**: 32-bit milliseconds gives only ~24.8 day max duration. This is standard for IEC 61131-3 Edition 2 but may surprise users accustomed to the current microsecond precision.
3. **VM cycle_time**: The VM passes cycle_time as i64 microseconds. Timer intrinsics must convert to milliseconds for PT/ET comparison. The hidden `start_time` field stays i64 to avoid precision loss in the conversion.
4. **DurationLiteral**: The DSL's `DurationLiteral` uses `time::Duration` which has nanosecond precision. The codegen truncates to the target unit (ms for TIME, us for LTIME). Sub-millisecond duration literals like `T#500us` would truncate to 0 for TIME.
