# Dual-Signature Integrity Model

status: proposed
date: 2026-02-18

## Context and Problem Statement

PLC bytecode travels from an engineering workstation to the PLC over a network. Historical attacks demonstrate that this channel is a critical attack surface:

- **Stuxnet (2010)** replaced PLC bytecode to destroy uranium centrifuges, while intercepting reads to display the original (benign) code to engineers
- **Rogue7 (2019)** demonstrated a rogue engineering station downloading arbitrary bytecode to Siemens S7-1500 PLCs while maintaining the displayed source code
- **CVE-2022-1161 (CVSS 10.0)** showed that Rockwell Logix controllers stored bytecode and source separately, allowing an attacker to modify one without the other

The bytecode container format must protect against tampering. How should integrity and authenticity be enforced?

## Decision Drivers

* **Bytecode-source binding** — an engineer must be able to verify that the bytecode running on the PLC corresponds to the source code they approved
* **Strippable debug info** — debug information (source line mappings, variable names) should be removable for production deployment without invalidating the integrity check
* **Independent trust chains** — the content (bytecode) and debug info may come from different sources or be signed at different times; the integrity model should not force them into a single signature
* **Incident response** — when debug info is present (e.g., during troubleshooting), it should be tamper-evident so an investigator can trust the line mappings and variable names
* **Constrained targets** — signature verification must work on micro PLCs (no TLS stack required, just a signature check)

## Considered Options

* No integrity protection — trust the network
* Single signature over everything (bytecode + debug info)
* Single signature over bytecode only — debug info is unsigned
* Dual signatures — content signature and debug signature, independently verifiable

## Decision Outcome

Chosen option: "Dual signatures", because it provides tamper-evident integrity for both content and debug info while allowing debug info to be stripped or replaced independently.

The model has three cryptographic elements:

1. **Content hash** (SHA-256) — covers the type section, constant pool, and code section. This is the hash that determines execution behavior.
2. **Source hash** (SHA-256) — a hash of the source text that produced this bytecode. Stored inside the content hash scope (in the file header), so it cannot be modified without invalidating the content signature. An engineer can independently hash their source and compare.
3. **Debug hash** (SHA-256) — covers the debug section (source line mappings, variable names, optionally the full source text). This is independent of the content hash.

Two signatures:

- **Content signature** — signs the content hash. Required. The PLC rejects bytecode without a valid content signature.
- **Debug signature** — signs the debug hash. Optional. Present only when debug info is present. Can use a different signing key than the content signature.

### Consequences

* Good, because stripping debug info does not invalidate the content signature — production deployments can remove debug info without re-signing
* Good, because the source hash inside the content hash prevents Stuxnet/Rogue7-style attacks — the PLC can attest that its bytecode was compiled from a specific source
* Good, because debug info is tamper-evident when present — an investigator can verify that line mappings and variable names haven't been manipulated
* Good, because independent signatures allow different trust chains — the build system signs the content, the developer signs the debug info, or a third-party tool generates and signs debug info separately
* Good, because the content signature is small and cheap to verify — a single Ed25519 or ECDSA-P256 signature check, feasible on micro PLCs
* Bad, because key management is required — the PLC must have a trusted public key (or certificate) to verify signatures against
* Bad, because two signatures means two verification checks at load time — though the debug signature is optional and only verified when debug info is used
* Bad, because the source hash is only useful if the engineer has access to the original source and computes the hash independently — it's a verification mechanism, not a display mechanism
* Bad, because the source hash does not protect against a Stuxnet-style attacker who controls the communication channel between the PLC and the engineer — if the attacker can intercept reads from the PLC, they can present a fake source_hash. The source_hash is effective against offline tampering (someone modifies the bytecode file on disk) but not against an attacker with MITM access to the PLC communication protocol. An out-of-band verification channel (e.g., physical access to read flash contents) is needed for the strongest assurance.

### Confirmation

Verify by:
1. Signing a bytecode container, stripping the debug section, and confirming the content signature still verifies
2. Modifying one byte of the code section and confirming the content signature rejects it
3. Modifying one byte of the debug section and confirming the debug signature rejects it while the content signature still verifies
4. Confirming the source hash in the header matches a SHA-256 computed over the original source text
5. Attempting to replace the source hash in the header and confirming the content signature rejects it

## Pros and Cons of the Options

### No Integrity Protection

Trust the network. No signatures, no hashes.

* Good, because implementation is trivial — no crypto code needed
* Bad, because this is how Stuxnet, Rogue7, and CVE-2022-1161 worked — the attacks would not have been possible with integrity protection
* Bad, because network protocols between engineering workstations and PLCs are frequently reverse-engineered (Siemens S7CommPlus was reverse-engineered for Rogue7)

### Single Signature Over Everything

One signature covers bytecode, constants, type metadata, and debug info.

* Good, because the integrity model is simple — one signature, one verification
* Bad, because stripping debug info invalidates the signature — production deployments must either keep debug info (wasting flash) or re-sign (requiring access to the signing key at deployment time)
* Bad, because the signing key must be available whenever debug info is modified — even adding a source line mapping to aid debugging requires re-signing the entire package

### Single Signature Over Bytecode Only

One signature covers bytecode, constants, and type metadata. Debug info is unsigned.

* Good, because debug info can be freely stripped or modified
* Bad, because an attacker can replace debug info with misleading content — variable names that don't match the actual variables, line mappings that point to wrong source lines
* Bad, because this undermines incident response — an investigator cannot trust the debug info in a container that may have been tampered with

### Dual Signatures (chosen)

Independent content signature and debug signature.

* Good, because each section's integrity is independently verifiable
* Good, because debug info can be stripped without affecting content integrity
* Good, because debug info is tamper-evident when present
* Good, because different signing keys can be used for content and debug
* Bad, because two verification checks are needed (when debug info is present)
* Bad, because the key management surface is larger (two keys instead of one)

## More Information

### Signature scope diagram

```
┌───────────────────────────────────────────────┐
│ File Header                                   │
│  content_hash ───────────────────────────┐    │
│  debug_hash ──────────────────────┐      │    │
│  source_hash ──────────────┐      │      │    │
│                            │      │      │    │
│  Content Signature ════════│══════│══╗   │    │
│   signs: content_hash      │      │  ║   │    │
│                            │      │  ║   │    │
│  Debug Signature ══════════│══╗   │  ║   │    │
│   signs: debug_hash        │  ║   │  ║   │    │
├────────────────────────────┼──╬───┼──╬───┘    │
│ Type Section ──────────────┼──╫───┼──╠══ covered by
│ Constant Pool ─────────────┼──╫───┼──╠══ content_hash
│ Code Section ──────────────┼──╫───┼──╝        │
├────────────────────────────┼──╬───┘           │
│ Debug Section ─────────────┼──╝               │
│  (source lines, var names) │  covered by      │
│                            │  debug_hash      │
│  source_hash ──────────────┘                  │
│   embedded in header,                         │
│   covered by content_hash                     │
└───────────────────────────────────────────────┘
```

### Recommended signature algorithms

| Algorithm | Key size | Signature size | Verification cost | Notes |
|---|---|---|---|---|
| Ed25519 | 32 bytes | 64 bytes | ~10 ms on Cortex-M4 | Fast, small, constant-time; recommended for PLC use |
| ECDSA-P256 | 32 bytes | 64 bytes | ~20 ms on Cortex-M4 | NIST standard; better regulatory compliance |
| RSA-2048 | 256 bytes | 256 bytes | ~100 ms on Cortex-M4 | Large signatures; slow on embedded; not recommended |

Ed25519 is recommended as the default. ECDSA-P256 as an alternative for environments requiring NIST-approved algorithms.

### Key distribution

This ADR defines the signature model but does not specify key distribution. Key distribution options (pre-provisioned keys, certificate chains, key rotation) are deployment concerns that should be addressed in a separate specification. The container format should support multiple signature algorithms via an algorithm ID byte, so the key distribution model can evolve independently.

### Interaction with ADR-0006 (Bytecode Verification)

On targets that perform on-device verification (ADR-0006), both the verifier and the signature are checked — defense-in-depth. On constrained targets that use the signature fallback, the content signature is the sole integrity guarantee. The dual-signature model ensures that even the fallback path provides meaningful protection.
