# Create a New Steering File

Create a new steering file for AI assistants. See [specs/steering/steering-file-guidelines.md](../../specs/steering/steering-file-guidelines.md) for the full guidelines and writing style.

## Steps

1. **Create the detailed doc**: `specs/steering/[topic].md`
   - Make it self-contained and AI-tool agnostic
   - Use directive style: "Always use X", "Never do Y"
   - Include code examples of good and bad patterns

2. **Decide: steering doc or skill?**
   - **Steering doc** (context/reference): Architecture, conventions, standards — loaded as background context
   - **Skill** (actionable procedure): Step-by-step tasks — invoked on-demand as a slash command

3. **Create Kiro pointer**: `.kiro/steering/[topic].md`
   ```markdown
   ---
   inclusion: always  # or fileMatch with pattern
   ---

   # [Topic Name]

   See [specs/steering/[topic].md](../../specs/steering/[topic].md) for the full [topic] guidance.

   [One sentence summary]
   ```

4. **Update Claude Code entry point**: Add reference to `CLAUDE.md`
   - For steering docs: add to the "Steering Files" section
   - For skills: create `.claude/commands/[action].md` with pointer and key commands

5. **Commit all files together** in a single commit

## Choosing Kiro inclusion strategy

- `inclusion: always` — Applies to most work (e.g., development-standards, common-tasks)
- `inclusion: fileMatch` with `fileMatchPattern` — Only for specific files (e.g., iec-61131-3-compliance for analyzer)
- `inclusion: manual` — Rarely needed, user must request with `#File`
