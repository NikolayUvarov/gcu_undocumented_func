# knowledge/ — MIND CORE project knowledge base

A folder for findings, verified facts and observations about the code. Each file covers one topic.
Improvement tasks live separately in `issues/`.

| File | Contents |
|------|-----------|
| [01-architecture-snapshot.md](01-architecture-snapshot.md) | What actually exists in the code: components, the `BootInfo` contract, control flow |
| [02-build-pipeline.md](02-build-pipeline.md) | Toolchain, build scripts, artifacts, what has been verified |
| [03-flat-binary-layout-analysis.md](03-flat-binary-layout-analysis.md) | Analysis of the ELF and flat binaries: `_start` offset, GOT, `.bss` |
| [04-handoff-vs-code-matrix.md](04-handoff-vs-code-matrix.md) | Comparison of the handoff document with the code, links to issues |
| [05-observations-and-risks.md](05-observations-and-risks.md) | Other observations and risks not covered by issues, or covered only partially |
| [06-apple-security-lessons.md](06-apple-security-lessons.md) | Apple's platform security mechanisms, their equivalents on Intel, AMD and ARM, what MIND Core has, and what it could take (proposals, in priority order) |

Maintenance rules:
- A fact is recorded only after it has been verified (a command plus its output, or a reference to a line of code).
- If a fact becomes outdated, the file is corrected rather than having a contradiction appended.
- Revision date: 2026-09-17, commit `8ad7550` (added howto build).
