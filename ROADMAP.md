# IronPLC Strategic Roadmap: The Sovridium Evolution

## 🛰️ Vision: The E3 Lattice

IronPLC is the foundational engine enabling the **E3 Ecosystem (Endogenous Eudaimonic Ecosystem)**. While maintaining its core as an open-source toolchain, it serves as a critical substrate for **Deterministically Observable Infrastructure**.

- **Endogenous**: Project development is local-first, rejecting SaaS dependency in favor of independent, self-hosted kernels.
- **Eudaimonic**: We empower human flourishing through autonomous excellence—using AI to manage technical complexity while maintaining sovereign control.

---

## 🛠️ Methodology: From SDD to IDD

We are evolving from **Spec-Driven Development (SDD)** to **Intent-Driven Development (IDD)**.

### 1. The SDD Foundation

Current development is grounded in the [Spec-Driven Adoption Plan](specs/plans/2026-04-10-spec-driven-adoption.md). We maintain high-integrity through:

- **Requirements Layer**: Mandated `REQ-AREA-NNN` format in `specs/requirements/`.
- **Conformance Tests**: Mandatory `{area}_spec_{claim}` naming in Rust tests.

### 2. The IDD Evolution (Synthesis Engine)

IDD builds upon SDD by automating the **Synthesis** (the "How") while the Human focuses on **Intent** (the "What" and "Why").

### 3. The Agentic Soul (Quality Gates)

"Agentic Soul" is measured through concrete project standards:

- **Edge Case Resilience**: Robust [Problem Code Management](specs/steering/problem-code-management.md).
- **Human Taste**: Adherence to the [Development Standards](specs/steering/development-standards.md).
- **Industrial Divinity (Rank Ω)**: Enforced via **85% code coverage**, **Clippy silence**, and **BDD-style test naming**.

---

## 🛰️ Strategic Focus: Sovridium™

**Sovridium** is the Sovereign Element of Control, bridging hardware telemetry with AI-native orchestration.

- **Intelligence**: Implementing [SAGE (Skill Augmented GRPO)](SAGE.md) for self-correcting industrial logic.
- **Integrity**: Local-first operation within the E3 lattice.

### 🔄 [The Proving Loop](SAGE.md#the-proving-loop)

SAGE operates in a continuous cycle to ensure industrial-grade reliability:

1. **Anomaly Proposer**: An agent creates "chaos" or failure scenarios in a digital twin.
2. **Engineer Agent**: Uses SAGE to compose **Skills** to resolve the anomaly.
3. **The Group Test**: Multiple skill combinations are simulated and tested simultaneously.
4. **GRPO Optimization**: Logic that performs better than the group average is reinforced ("bred for resilience"), while unstable paths are discarded.

---

## 🗣️ Feedback Ecosystem: The Five Voices

We iterate based on structured feedback loops. Mechanisms for data collection are currently under research.

- **Voice of the Customer (VoC)**: End-user experience (Playground/Extension).
- **Voice of the Integrator (VoI)**: System Integrators and EPC (Engineering Procurement Construction) feedback for **Brownfield** (modernization) and **Greenfield** (new-build) automation systems.
- **Voice of the Developer (VoD)**: Contribution workflow and IDD efficiency.
- **Voice of the Agent (VoA)**: Derived from [MCP Structured Logs](specs/design/mcp-server.md#logging-and-observability).
- **Customer Advisory Board (CAB)**: Strategic strategic alignment and industrial compliance.

---

## 📍 Current Milestones (2026)

- [x] **Phase 0**: Launch the [Spec-Driven Adoption Plan](specs/plans/2026-04-10-spec-driven-adoption.md).
- [ ] **Phase 1**: Complete the 13-phase [MCP Server Implementation](specs/plans/2026-04-14-mcp-server-plan.md).
- [ ] **Phase 2**: SAGE (GRPO) integration for VM safety-critical scheduling.
- [ ] **Phase 3**: IDD workflow standardization across the [Agentic Registry](AGENTS.md).

---

## 🔦 Research Roadmap: Future World Mapping

Deep research is required to map the long-term navigation routes of the E3 Lattice.

### 🔭 Navigation Route 1: Autonomous Safety Logic

Researching the application of **SAGE (Skill Augmented GRPO)** to autonomously verify safety-critical PLC logic against industrial benchmarks.

### 🔭 Navigation Route 2: The Sovereign Infrastructure Lattice

Establishing the "Future World" aspects of local-first deployment—bridging the gap between the workstation gateway and SBC substrates without external dependencies.

### 🔭 Navigation Route 3: Human-Agent Co-Synthesis

Optimizing the navigation route between **Human Intent** and **Agentic Execution**, ensuring that the AI "Harness" remains a force for human flourishing.

### 🔭 Navigation Route 4: The Five Voices of Quality ("Good Taste")
Deep research into defining specific Quality Gates for the feedback ecosystem, focusing on "Good Taste" and aesthetic excellence:
- **VoC (Customer)**: Researching metrics for "Intuitive Soul"—minimal friction onboarding and playground aesthetic.
- **VoI (Integrator)**: Defining "Industrial Divinity" for **Systems Integration (ASI)**—researching deterministic deployment across both **Brownfield** (legacy modernization) and **Greenfield** (new-build) control systems, including back-testing stability and legacy compatibility.
- **VoD (Developer)**: Optimizing for "Cognitive Flow"—CI speed, steering clarity, and Rank Ω code aesthetics.
- **VoA (Agent)**: Exploring "Synthesis Fidelity"—High-context MCP vocabulary and **Evergreen Feedback** loops where agents garden the SSOT through documented **Lessons Learned**.
- **CAB (Advisory Board)**: Mapping **"Strategic Integrity"**—Industrial compliance benchmarks (**HazOp**, Flight-Safety) and long-term ecosystem stability.

### 🔭 Navigation Route 5: Software-Defined Safety (The Rocket-Grade Gate)
Deep research into converting **HazOp**, **ISA-88 (Batch Control)**, and **GAMP 5 / FDA 21 CFR Part 11 (Pharma/Food Safety)** standards into deterministic code-level quality gates for **Software-Defined Automation Systems**. Focus on creating an immutable **VCAT (Version Control & Audit Trail)** substrate.

### 🔭 Navigation Route 6: The Kinetic Update (75MPH Tire Change)
Research into the methodologies of "Changing the tire at 75MPH"—upgrading live control systems with zero downtime:
- **Bumpless Transfer**: Architecting state-mirroring and hand-off protocols for seamless logic swaps.
- **Lock-Step Synchronization**: Ensuring signal-event alignment during live hot-reloads.
- **Shadow Mode Verification**: Running new control logic in a non-authoritative "Shadow" state before commitment to the physical process.
