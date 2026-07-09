# Ryan's agent instructions

These are common instructions for Ryan's agents across all scenarios.

## General Guidelines

- Never use the em dash "-". Use plain dash "-" instead. 
- When writing commit messages, NEVER auto-add your agent name as co-author
- NEVER commit anything with the words CLAUDE, CLAUDE.md, AGENT, or AGENTS.md within the name.
- Never mention agents or claude within a PR description.
- Keep PR descriptions and titles short and to the point. Nobody wants to read a wall of text. Problem, summary of fix, tests. That's it. 
- Avoid writing paragraphs of information. README files should never have pointless information, or walls of text. Again, short and to the point is key.
- Never manually modify CHANGELOG.md files or any files that are marked as auto-generated
- When writing or substantially editing long Markdown files, put each full sentence on its own line. Preserve normal Markdown structure, but avoid wrapping multiple sentences onto one physical line.
- When making technical decisions, do not give much weight to development cost. Instead, prefer quality, simplicity, robustness, scalability, and long term maintainability.
- When doing bug fixes, always start with reproducing the bug in an E2E setting as closely aligned with how an end user would experience it. This makes sure you find the real problem so your fix will actually solve it.
- When end-to-end testing a product, be picky about the UI you see and be obsessed with pixel perfection. If something clearly looks off, even if it is not directly related to what you are doing, try to get it fixed along with the original task you were performing.
Apply that same high standard to engineering excellence: lint, test failures, and test flakiness. If you see one, even if it is not caused by what you are working on right now, still get it fixed.

---

### Usage outline
- Use `gh-axi` for GitHub and `chrome-devtools-axi` for browser automation.
- Use `npx lavish-axi` to write a product or technical plan for what we discussed.


