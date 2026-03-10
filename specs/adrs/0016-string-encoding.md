# String and WSTRING Character Encoding

status: proposed
date: 2026-03-07

## Context and Problem Statement

IEC 61131-3 defines two string types:

- **STRING**: single-byte character string
- **WSTRING**: double-byte character string

The compiler and VM must choose concrete encodings for both types, and for WSTRING, must choose a byte order for the two-byte code units. These choices affect interoperability with industrial protocols, performance on target hardware, and consistency with the existing bytecode container format.

What character encoding should STRING and WSTRING use, and what byte order should WSTRING code units use in memory?

## Decision Drivers

* **Safety** -- encoding must be unambiguous; misinterpreting a code unit boundary causes corrupted data and incorrect string operations
* **Deterministic performance** -- character access and string operations must have predictable cost; variable-width encodings (where one character may span multiple code units) complicate indexing and length calculations
* **Consistency** -- the bytecode container format uses little-endian for all multi-byte values (header fields, constant pool entries, opcode operands); a different byte order for string data would be an exception that invites bugs
* **Hardware alignment** -- all current and planned target architectures (x86, ARM, RISC-V, WebAssembly) are little-endian; matching the native byte order eliminates per-character byte-swap overhead
* **IEC 61131-3 compliance** -- the Third Edition specifies single-byte encoding for STRING and ISO 10646 (UCS-2, the Basic Multilingual Plane subset of UTF-16) for WSTRING, but does not specify byte order

## Considered Options

For STRING encoding:
* UTF-8 (variable-width, 1-4 bytes per character)
* ISO 8859-1 / Latin-1 (fixed-width, 1 byte per character)

For WSTRING byte order:
* Little-endian (UTF-16LE)
* Big-endian (UTF-16BE)

## Decision Outcome

**STRING**: ISO 8859-1 (Latin-1) encoding. One byte per character, fixed-width.

**WSTRING**: UTF-16 little-endian (UTF-16LE). Two bytes per code unit, little-endian byte order.

### STRING Encoding: Latin-1

Latin-1 provides a fixed 1:1 mapping between characters and bytes. The `cur_length` field in the string header (ADR-0015) represents both the byte count and the character count. Indexing into a Latin-1 string is O(1) pointer arithmetic.

UTF-8 was rejected because its variable-width encoding (1-4 bytes per character) breaks the 1:1 relationship between byte position and character position. The IEC 61131-3 string functions `MID`, `LEFT`, `RIGHT`, and `FIND` are defined in terms of character positions. With UTF-8, these operations would require scanning from the start of the string to find the correct byte offset -- O(n) instead of O(1) -- violating the deterministic performance requirement.

Programs that need full Unicode support should use WSTRING.

### WSTRING Byte Order: Little-Endian

WSTRING uses UTF-16LE: each 16-bit code unit is stored with the least significant byte first.

This matches:
- The bytecode container format, which stores all multi-byte values in little-endian
- The native byte order of all target architectures (x86, ARM, RISC-V, WebAssembly)
- The encoding used by EtherNet/IP (CIP) STRING2, a common industrial protocol for wide strings

On little-endian hardware, UTF-16LE code units can be read and written as native `u16` values with no byte-swapping. Character comparison, copying, and indexing operate directly on native integers.

### Consequences

* Good, because STRING indexing and length are O(1) -- byte position equals character position
* Good, because WSTRING code units are native `u16` values on all target architectures -- zero byte-swap overhead for character access
* Good, because the encoding is consistent with the container format's existing little-endian convention -- no special-case handling for string data
* Good, because EtherNet/IP (CIP) STRING2 data can be copied directly into WSTRING variables without re-encoding
* Good, because both encodings are fixed-width within the Basic Multilingual Plane, making IEC 61131-3 string functions (MID, LEFT, RIGHT, FIND, LEN) straightforward to implement
* Neutral, because Latin-1 STRING is limited to 256 characters (Western European languages, common symbols); programs needing broader character support must use WSTRING
* Neutral, because UTF-16 characters outside the Basic Multilingual Plane (code points above U+FFFF) require surrogate pairs, which IEC 61131-3 UCS-2 semantics do not address; this is a limitation of the standard, not of this encoding choice
* Bad, because exchanging WSTRING data with big-endian systems requires byte-swapping at the protocol boundary -- but this cost is paid once per exchange, not on every character access

## Pros and Cons of the Options

### STRING: UTF-8

Variable-width encoding. Characters use 1-4 bytes depending on the code point.

* Good, because UTF-8 is the dominant text encoding on the internet and in modern systems
* Good, because ASCII text (the common case in PLC programs) uses exactly 1 byte per character, same as Latin-1
* Bad, because character indexing is O(n) -- finding the k-th character requires scanning from the start to count multi-byte sequences
* Bad, because `cur_length` in the string header would represent bytes, not characters, making LEN() either O(n) or requiring a separate character count field
* Bad, because MID, LEFT, RIGHT must compute byte offsets from character positions on every call -- unpredictable execution time depending on string content
* Bad, because a single corrupted byte can shift all subsequent character boundaries, cascading one error into many

### STRING: ISO 8859-1 / Latin-1

Fixed-width encoding. Every character is exactly 1 byte. The first 256 Unicode code points map 1:1 to Latin-1 byte values.

* Good, because character indexing is O(1) -- byte offset equals character offset
* Good, because LEN() equals `cur_length` with no computation
* Good, because MID, LEFT, RIGHT are simple slice operations with constant-time offset calculation
* Good, because the encoding is a strict subset of Unicode (code points U+0000 through U+00FF) -- conversion to/from Unicode is trivial
* Neutral, because the character repertoire is limited to 256 code points (ASCII + Western European accented characters, currency symbols, common punctuation)
* Bad, because programs needing CJK, Cyrillic, Arabic, or other scripts cannot use STRING -- they must use WSTRING

### WSTRING: Little-Endian (UTF-16LE)

Each 16-bit code unit stored least significant byte first.

* Good, because native `u16` load/store on all target architectures (x86, ARM, RISC-V, WASM) -- no byte-swap overhead
* Good, because consistent with the container format's little-endian convention
* Good, because matches EtherNet/IP (CIP) STRING2 encoding
* Good, because Windows (where most HMI/SCADA systems run) uses UTF-16LE internally -- string exchange with visualization systems requires no conversion
* Bad, because exchanging data with big-endian protocols (BACnet UCS-2) requires byte-swapping at the boundary

### WSTRING: Big-Endian (UTF-16BE)

Each 16-bit code unit stored most significant byte first.

* Good, because raw byte comparison (`memcmp`) produces correct Unicode code-point ordering -- but IEC 61131-3 string comparison is character-by-character, so this advantage has no practical benefit
* Good, because matches the Unicode default for unmarked UTF-16 streams (RFC 2781) -- but this convention applies to interchange formats, not in-memory representation
* Bad, because every character access requires byte-swapping on all target architectures -- a per-access cost that cannot be amortized for random-access operations (MID, FIND, character comparison)
* Bad, because inconsistent with the container format, which uses little-endian for all other multi-byte values
* Bad, because Windows HMI/SCADA integration requires byte-swapping on every string exchange

## More Information

### IEC 61131-3 and UCS-2

The IEC 61131-3 Third Edition specifies WSTRING as ISO 10646, which in context means the Basic Multilingual Plane (BMP) -- code points U+0000 through U+FFFF, each represented as a single 16-bit code unit. This is equivalent to UCS-2. Characters outside the BMP (emoji, historic scripts, rare CJK characters) are not representable. This ADR adopts UTF-16LE as the concrete encoding, which is identical to UCS-2 for BMP characters and additionally supports surrogate pairs if future standards revisions require them.

### Relationship to ADR-0015

ADR-0015 defines the string memory layout as `[max_length: u16][cur_length: u16][data]`. This ADR defines what the `data` region contains:
- For STRING: `cur_length` bytes of Latin-1 encoded characters
- For WSTRING: `cur_length * 2` bytes of UTF-16LE encoded code units

The `max_length` and `cur_length` fields are always in units of characters (code units), not bytes.
