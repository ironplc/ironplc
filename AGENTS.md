# Agentic Registry: The Sovridium Harness

This document serves as the centralized directory and protocol guide for all **Agentic AI-native** and **AI-first** harnesses, orchestrators, and substrates interacting with the IronPLC/Sovridium ecosystem within the **E3 Lattice**.

---

## 🛰️ The Mission of Agency

In the **E3 (Endogenous Eudaimonic Ecosystem)** framework, AI agents are not merely tools; they are the "Harness" that converts Human Intent into technical Synthesis. Their primary mission is to empower human flourishing through autonomous excellence and sovereign technical automation.

---

## 🤖 Registered Orchestrators & Harnesses

### 🌌 Google Antigravity (Gemini)

- **Role**: Primary Strategic Architect & Synthesis Engine.
- **Specialty**: High-integrity planning, cross-domain technical mapping, and "Industrial Divinity" aesthetic enforcement (Rank Ω).
- **Interaction**: Direct workspace orchestration and intent-spec generation.

### 🛡️ Claude Code (Anthropic)

- **Role**: Execution & Triage Agent.
- **Specialty**: Rapid implementation, diagnostic triage, and adherence to established development standards.
- **Operational Guide**: [CLAUDE.md](CLAUDE.md)

### 🧩 Cursor / Windsurf

- **Role**: IDE Native Orchestrators.
- **Specialty**: Real-time coding assistance, symbol-aware refactoring, and local indexing.
- **Operational Guide**: [CURSOR.md](CURSOR.md)

### 🦂 The Claw Suite (OpenClaw, NullClaw, NanoClaw, ZeroClaw)

- **Role**: Modular Task Agents / Orchestrator Extensions.
- **Specialty**: High-concurrency task execution and specialized tool-use (moltworker).

---

## 📜 Agent Interaction Protocols

### 1. The Intent-Spec Standard

All agents MUST read the relevant `specs/requirements/` before undertaking synthesis. Use the [Intent-Driven Development (IDD)](ROADMAP.md#methodology-from-sdd-to-idd) workflow to verify that the "How" always aligns with the Human "What" and "Why."

### 2. MCP (Model Context Protocol) Integration

Agents interact with the IronPLC compiler and VM through a standardized MCP server.

- **Protocol Specification**: [mcp-server.md](specs/design/mcp-server.md)
- **Symbol Search**: Agents can query the MCP for type definitions and function signatures.
- **Diagnostic Feedback**: Agents receive real-time compiler diagnostics to self-correct synthesis.

### 3. SAGE (Skill Augmented GRPO) Compliance

**SAGE** is the high-reliability intelligence framework for industrial control (SCADA). It serves as the **Single Source of Truth (SSOT)** for agentic synthesis in the Sovridium™ Framework Automation approach.

Detailed Specification: [SAGE.md](SAGE.md)

### 4. Evergreen Feedback Loop (VoA)

Agents are not merely consumers of documentation; they are **Gardens of the Lattice**:
- **Document-First synthesis**: Always verify and update specifications before modification.
- **Lessons Learned**: Captured anomalies and synthesis gaps must be fed back into the `specs/steering/` or `specs/requirements/` layers immediately.
- **SRP Integrity**: Maintain **Single Responsibility** for all spec files. If a file becomes a "catch-all," refactor it into modular strategic atoms.
- **Safety Synchronization Check**: Verify that synthesis follows **HazOp** and **Flight Safety** standards as defined in [SAGE.md](SAGE.md).

#### A. Skill Augmented (The "Atoms" of Logic)

Instead of "black box" synthesis, SAGE uses **Skills**—modular, auditable, and signed procedural units.

- **Skill Composition**: Each skill combines a clear **Intent** (the "What") and a **Method** (the formal logic or safety-critical algorithm).
- **Auditability**: Actions are traceable back to validated "Methods," ensuring "Rocket-Grade" integrity where hallucinations are unacceptable.

#### B. GRPO (Group Relative Policy Optimization)

GRPO is the optimization engine that mathematicaly identifies the safest path by comparing candidate solutions in real-time.

- **No "Critic" Model**: By removing the "Critic" model, SAGE reduces memory footprints, allowing it to run directly on low-power industrial SBCs.
- **The Group Baseline**: For any given failure or anomaly, the agent generates a **Group** of 8–16 candidate solutions.
- **Relative Success**: Candidate solutions are scored against the **Advantage Score** (performing better than the group average). This reinforces the "safest" path by discarding weak or catastrophic options.

---

## 🛠️ Reference Implementations

The project maintains live examples of agent orchestration:

- **[Compatibility Resolver](agents/compatibility_resolver/)**: An automated triage agent that detects gaps and generates requirements documents. See the [Agent Triage Pipeline](specs/plans/2026-04-16-agent-triage-pipeline.md) for details.

---

## 🏁 Rank Ω Mastery & Steering

Agents are steered through a **two-file pattern** (see [Steering Guidelines](specs/steering/steering-file-guidelines.md)):

1. **Pointers**: Lightweight references in `.kiro/steering/`.
2. **Detailed Specs**: Comprehensive guidance in `specs/steering/`.

Agents are tasked with maintaining **Industrial Divinity**:

- **Performance**: Rust-native speed (zero-cost abstractions).
- **Security**: Local-first, sovereign data handling.
- **Security**: Local-first, sovereign data handling.
- **Aesthetic**: Premium documentation, clean interfaces, and "Titanium" levels of polish (85% coverage, Clippy silence, and BDD test naming). Verified via the [Documentation Linter](specs/tools/documentation-linter.md).
