# Specification: Documentation Linter (`lint-docs.sh`)

## 🛰️ Intent
The **Documentation Linter** is a high-integrity verification tool designed to maintain "Industrial Divinity" across the Sovridium lattice. It ensures that all strategic documentation (ROADMAP, SAGE, AGENTS, etc.) remains structurally consistent, geometrically stable, and formatted to the Rank Ω project standard.

---

## 🛡️ Design Principles

### 1. Risk-Aversion
The tool is built on a "minimal mutation" substrate. It prioritizes system sovereignty by defaulting to read-only audits. Automated system changes (such as dependency installation) are restricted to the project-local scope and require explicit human authorization.

### 2. Deterministic Intent
Every operation performed by the tool is signaled by an **Intent Header** (e.g., `Intent: RO-Audit`). The tool verifies its operational context (project root) before execution to eliminate path ambiguity and ensure predictable outcomes.

### 3. Security-First
- **Deterministic Choice Menu**: Offers an explicit selection between **Local**, **Global**, or **Abort** installation paths.
- **Transparent Risk Disclosure**: Clearly defines the impact of any proposed change before asking for permission.
- **Fail-Safe Defaults**: Non-interactive environments automatically block all mutation paths.
- **Root-Avoidance**: Prefers project-local node modules to maintain system-wide sovereignty.

---

## 🏗️ Architecture

### Substrate
- **Language**: Bash (POSIX compliant where possible).
- **Core Engine**: `markdownlint-cli` (Node.js).
- **Configuration**: [`.markdownlint.yaml`](../../.markdownlint.yaml) define the specific geometric rules of the lattice.

### The Proving Loop (Verification)
1.  **Context Check**: Verifies `justfile` and `docs/` existence.
2.  **Runtime Check**: Verifies `node` availability.
3.  **Linter Check**: Screens for `markdownlint` in `./node_modules/.bin` and system `$PATH`.
4.  **Audit Execution**: Scans `*.md` in root and `docs/**/*.md` recursively.

---

## 📜 Usage & Automation

### Standard Automation
The linter is integrated into the root `justfile` as a primary audit recipe:
```bash
just lint-docs
```

### Direct Execution
For manual verification or restricted environments:
```bash
sh tools/lint-docs.sh
```

---

## 🧠 Lessons Learned (VoA)

The development of this tool has provided critical insights into the maintenance of a high-integrity strategy lattice:

1.  **Configuration Resilience**: Strategic tools must validate their own **Configuration Substrate** (e.g., `.markdownlint.yaml`) before execution to prevent cascading failures due to syntax corruption.
2.  **Geometric Flexibility**: Industrial-grade documentation often involves complex tactical sentences that exceed standard 80-character line limits. Tools must allow for **Strategic Rule Suppression** (e.g., disabling MD013) to maintain "Cognitive Flow" over rigid formatting.
3.  **Automatic Rectification**: Future iterations of the linter should prioritize **Autonomous Remediation** (e.g., `markdownlint --fix`) to reduce the manual overhead of aesthetic maintenance.
4.  **Signal Extraction**: Linter failures are not just errors; they are **Feedback Signals** for the VoA (Voice of the Agent) loop, identifying areas where the human intent and tool standards have diverged.

---

## 🏁 Compliance
All documentation commits MUST pass the `lint-docs` audit before being merged into the `main` branch. This is enforced by the **Agentic Soul** quality gates (Rank Ω).
