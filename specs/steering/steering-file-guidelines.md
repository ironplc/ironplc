# Steering File Guidelines

This document explains how to create and maintain steering files for the IronPLC project. Steering files guide AI assistants (like Kiro, Claude, GitHub Copilot, Cursor, etc.) to work effectively with the codebase.

## The Pointer Pattern

IronPLC uses a **pointer pattern** for steering files to maximize compatibility across different AI systems:

### Core Principle: Single Source of Truth

All detailed steering documentation lives in **`specs/steering/`**. Different AI tools reference this documentation through their own pointer mechanisms:

- **Kiro**: Uses pointer files in `.kiro/steering/` that are automatically loaded
- **Claude**: Uses `CLAUDE.md` as an entry point with links to `specs/steering/`
- **Cursor**: Could use `CURSOR.md` with links to `specs/steering/`
- **Other AI tools**: Can reference `specs/steering/` files directly or through custom entry points

### 1. Detailed Documentation (in `specs/steering/`)

**Purpose**: Complete guidance that works standalone for any AI system

**Location**: `specs/steering/[topic].md`

**Format**: Full markdown documentation with all details

**Key characteristics**:
- Self-contained (doesn't rely on any specific AI tool's features)
- Can be referenced by any AI system through their pointer mechanism
- Works when copied/pasted into any AI chat
- Contains all the actual guidance content
- Single source of truth for all AI tools

### 2. AI Tool-Specific Pointers

Different AI tools use different mechanisms to reference the detailed documentation:

#### Kiro Pointer Files (`.kiro/steering/`)

**Purpose**: Lightweight references that Kiro loads automatically

**Location**: `.kiro/steering/[topic].md`

**Format**:
```markdown
---
inclusion: always  # or fileMatch with pattern
---

# [Topic Name]

See [specs/steering/[topic].md](../../specs/steering/[topic].md) for the full [topic] guidance.

[One sentence describing what this guidance covers]
```

**Example** (`.kiro/steering/common-tasks.md`):
```markdown
# Common Development Tasks

See [specs/steering/common-tasks.md](../../specs/steering/common-tasks.md) for the full common tasks reference.

This file provides quick reference for build commands, testing workflows, and development tasks using the project's justfile-based build system.
```

#### Claude Entry Point (`CLAUDE.md`)

**Purpose**: Entry point document that Claude Code reads automatically

**Format**:
```markdown
# Claude Code Instructions

## Steering Files

Before making changes, read the relevant steering files in `specs/steering/`:

- **[Topic Name](specs/steering/[topic].md)** - [Brief description]
```

#### Other AI Tools

Other AI assistants can use similar entry point files (e.g., `CURSOR.md`, `COPILOT.md`) or directly reference files in `specs/steering/` through their own mechanisms

## Why This Pattern?

### Benefits

1. **AI System Agnostic**: The detailed docs in `specs/steering/` work with any AI system
2. **Single Source of Truth**: One detailed doc, multiple pointer mechanisms
3. **Efficient Token Usage**: Each AI tool can optimize how it loads the documentation
4. **Easy Maintenance**: Update one file in `specs/steering/`, all AI tools benefit
5. **Portable**: Works across different development environments and AI assistants
6. **Flexible Integration**: Each AI tool uses its native pointer mechanism

### Token Efficiency Example (Kiro)

Kiro's file-based pointer system provides efficient token usage:

```
Without pointer pattern:
- Kiro loads 5KB of detailed guidance automatically
- User asks simple question
- 5KB loaded but only 200 bytes needed
- Wasteful

With pointer pattern:
- Kiro loads 200 byte pointer automatically
- User asks simple question
- Pointer has enough info to answer
- If more detail needed, Kiro reads the full doc
- Efficient
```

### Cross-Tool Compatibility Example

The same detailed documentation works for all tools:

```
specs/steering/common-tasks.md (single source of truth)
    ↓
    ├─→ .kiro/steering/common-tasks.md (Kiro pointer)
    ├─→ CLAUDE.md (Claude entry point)
    ├─→ CURSOR.md (Cursor entry point, if created)
    └─→ Direct reference (any AI tool)
```

## Creating New Steering Files

### Step 1: Create the Detailed Documentation

Create `specs/steering/[topic].md` with complete guidance:

```markdown
# [Topic Name]

[Introduction explaining what this guidance covers]

> **Note**: This file provides detailed guidance for AI-assisted development. 
> For [related human docs], see [link].

## [Section 1]

[Detailed content...]

## [Section 2]

[Detailed content...]
```

### Step 2: Create Pointers for Each AI Tool

#### For Kiro

Create `.kiro/steering/[topic].md`:

```markdown
---
inclusion: always  # or conditional
---

# [Topic Name]

See [specs/steering/[topic].md](../../specs/steering/[topic].md) for the full [topic] guidance.

[One sentence summary of what this covers]
```

#### For Claude

Update `CLAUDE.md` to add a reference:

```markdown
## Steering Files

Before making changes, read the relevant steering files in `specs/steering/`:

- **[Topic Name](specs/steering/[topic].md)** - [Brief description]
```

#### For Other AI Tools

Create or update their entry point files (e.g., `CURSOR.md`, `COPILOT.md`) following the same pattern as `CLAUDE.md`.

## Kiro-Specific: Conditional vs. Always Inclusion

Kiro's pointer files support different inclusion strategies through frontmatter:

### Always Included (Default)

```yaml
---
inclusion: always
---
```

Use when:
- Guidance applies to most/all work in the repository
- Content is small and high-value
- Examples: development-standards, common-tasks

### Conditionally Included

```yaml
---
inclusion: fileMatch
fileMatchPattern: "**/analyzer/**"
---
```

Use when:
- Guidance only applies to specific files/directories
- Content is specialized
- Examples: iec-61131-3-compliance (only for analyzer), problem-code-management (only for problems/)

### Manual Inclusion

```yaml
---
inclusion: manual
---
```

Use when:
- Guidance is rarely needed
- Content is very large
- User should explicitly request it with `#File`

**Note**: Other AI tools can always reference `specs/steering/` files directly, regardless of Kiro's inclusion settings.

## Content Guidelines

### What Goes in Steering Files

✅ **DO include**:
- Architectural patterns and principles
- Code organization conventions
- Testing patterns and requirements
- Error handling approaches
- Build system usage
- Common workflows
- Project-specific terminology
- Cross-references to justfiles, configs, etc.

❌ **DON'T include**:
- Complete API documentation (that's for code comments)
- Entire configuration files (reference them instead)
- Step-by-step tutorials (that's for docs/)
- Changelog or version history
- Implementation details that change frequently

### Writing Style

**For AI assistants, not humans**:
- Be directive: "Always use X", "Never do Y"
- Include examples of good and bad patterns
- Explain the "why" behind rules
- Use code examples liberally
- Cross-reference other files rather than duplicating

**Example**:
```markdown
## Test Naming

**Always use BDD-style test names** following the pattern:
`function_name_when_condition_then_expected_result`

✅ **Good**:
- `validate_subrange_bounds_when_out_of_range_then_error`

❌ **Bad**:
- `test_subrange_1`
- `subrange_validation`
```

## Maintaining Steering Files

### When to Update

Update steering files when:
- Adding new architectural patterns
- Changing project conventions
- Adding new build commands or workflows
- Discovering common AI assistant mistakes
- Refactoring major components

### Update Checklist

When updating a steering file:

1. ✅ Update the detailed doc in `specs/steering/[topic].md`
2. ✅ Check if pointer summaries need updating:
   - `.kiro/steering/[topic].md` (if exists)
   - `CLAUDE.md` (if topic is listed)
   - Other AI tool entry points
3. ✅ Test with an AI assistant to verify the guidance works
4. ✅ Commit all updated files together

### Avoiding Duplication

**Don't duplicate content between**:
- Steering files and CONTRIBUTING.md
- Steering files and code comments
- Multiple steering files

**Instead**:
- Steering files guide AI assistants on "how to work with this codebase"
- CONTRIBUTING.md guides humans on "how to contribute"
- Code comments document "what this code does"
- Cross-reference rather than duplicate

**Example**:
```markdown
# In steering file
For complete setup instructions, see [CONTRIBUTING.md](../../CONTRIBUTING.md).

# In CONTRIBUTING.md
For AI-assisted development patterns, see `specs/steering/`.
```

## Examples from IronPLC

### Good: development-standards.md

- **Detailed**: `specs/steering/development-standards.md` (400+ lines)
- **Kiro pointer**: `.kiro/steering/development-standards.md` (3 lines)
- **Claude reference**: Listed in `CLAUDE.md`
- **Why it works**: Single source of truth, multiple access methods

### Good: problem-code-management.md

- **Detailed**: `specs/steering/problem-code-management.md`
- **Kiro pointer**: `.kiro/steering/problem-code-management.md` with `fileMatch`
- **Claude reference**: Listed in `CLAUDE.md`
- **Why it works**: Conditionally loaded in Kiro when working with problem files, always available to Claude

### Pattern to Follow

```
specs/steering/                          # Single source of truth
├── common-tasks.md                      # Detailed (200+ lines)
├── compiler-architecture.md             # Detailed (300+ lines)
├── development-standards.md             # Detailed (400+ lines)
├── iec-61131-3-compliance.md           # Detailed (150+ lines)
├── problem-code-management.md          # Detailed (100+ lines)
└── steering-file-guidelines.md         # This file

.kiro/steering/                          # Kiro-specific pointers
├── common-tasks.md                      # Pointer (always)
├── compiler-architecture.md             # Pointer (always)
├── development-standards.md             # Pointer (always)
├── iec-61131-3-compliance.md           # Pointer (fileMatch)
├── problem-code-management.md          # Pointer (fileMatch)
└── steering-file-guidelines.md         # Pointer (always)

CLAUDE.md                                # Claude entry point
                                         # Lists all specs/steering/ files
```

## AI Assistant Instructions

When an AI assistant is asked to create or update steering files:

1. **Always create detailed doc first** in `specs/steering/`
2. **Make it AI-tool agnostic** - no tool-specific features in the detailed doc
3. **Create appropriate pointers**:
   - For Kiro: Create `.kiro/steering/[topic].md` pointer
   - For Claude: Update `CLAUDE.md` with reference
   - For other tools: Update their entry point files
4. **Use appropriate inclusion strategy** (Kiro only: always, fileMatch, or manual)
5. **Keep pointers minimal** (3-5 lines max)
6. **Make detailed docs self-contained** (work without any specific AI tool)
7. **Test across AI tools** if possible

## Testing Steering Files

After creating or updating steering files:

1. **Test with multiple AI tools** (if available):
   - Kiro: Ask a question that should trigger the guidance
   - Claude: Copy the detailed doc into a chat and verify it's clear
   - Other tools: Test with their native access methods
2. **Check token usage** (if visible): Ensure pointers are efficient
3. **Verify cross-references**: Ensure links to justfiles, configs, etc. are correct
4. **Confirm AI-tool agnostic**: Detailed doc should not mention specific AI tools

## Questions?

If you're unsure whether something should be in a steering file:

- **Is it guidance for AI assistants?** → Yes, steering file (`specs/steering/`)
- **Is it setup instructions for humans?** → No, CONTRIBUTING.md
- **Is it API documentation?** → No, code comments
- **Is it a trade-off decision (why X over Y)?** → ADR (`specs/adrs/`)
- **Is it describing what to build (architecture, formats, interfaces)?** → Design doc (`specs/design/`)
- **Is it describing how to implement (phased tasks, code changes)?** → Implementation plan (`specs/plans/`)
- **Is it explaining a design decision for users?** → Maybe, consider `docs/explanation/`
- **Is it a common mistake AI makes?** → Yes, steering file (`specs/steering/`)
