//! Integration tests for `#[derive(U16Id)]`.

use container_derive::U16Id;

#[derive(Clone, Copy, Debug, PartialEq, Eq, U16Id)]
struct Sample(u16);

#[test]
fn new_and_raw_when_round_tripped_then_preserves_value() {
    assert_eq!(Sample::new(5).raw(), 5);
}

#[test]
fn to_le_bytes_when_called_then_little_endian_bytes() {
    assert_eq!(Sample::new(0x1234).to_le_bytes(), [0x34, 0x12]);
}

#[test]
fn display_when_formatted_then_writes_number() {
    assert_eq!(Sample::new(789).to_string(), "789");
}

#[test]
fn new_when_const_context_then_usable() {
    const VALUE: Sample = Sample::new(42);
    assert_eq!(VALUE.raw(), 42);
}
