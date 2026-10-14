# 000-KRN-0106 — Roadmap 1.10: its heading and its "Current state" say where the code is

**Type:** documents (text only; the roadmap in English and Russian, a new version) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.3, MC-12.5

## Problem

From the critique of 2026-10-14 ([issues-critica-done/01](../issues-critica-done/01_constitution_and_docs_analysis.md), §2), checked against `main` dfc42ee.

- **The heading.** ROADMAP.md:1 and ROADMAP_RU.md:1 say "Roadmap v1.1", while line 3 says "Version: 1.9".
- **Section 2, "Current state", describes the code before stage II.** Line 27, and in the table the rows I–III and VII, say:
  - SMP up to 8 CPUs; 32 capability slots per task;
  - slots without generations; no derivation tree and no `CAP_REVOKE`;
  - endpoint numbers global in the ABI;
  - no interface description language, no supervisor;
  - no VirtIO;
  - user services "will need porting after stage II".
- **Sections 3 and 6 of the same file mark K1, K3, K4 and C1–C8 done (2026-10-03/04).** README and the code show them:
  - up to 255 CPUs on x86 (xAPIC or x2APIC mode) and 256 on aarch64;
  - 104 slots at a task's start, growing to 4096, with 32-bit generations;
  - the derivation tree, `CAP_MINT` and `CAP_REVOKE`;
  - per-task handles;
  - MIND IDL, and `init` as the supervisor;
  - VirtIO devices on both architectures.
- **Lines of section 3 that read as the present:**
  - :58 "memory is still paid from the global kernel heap": capability tables, page tables and tasks are charged to their owners since issue 171;
  - :62 "24-bit generation… fixed slots 1–9": generations are 32-bit now, and the fixed slots are 1–25.

## Plan

1. **Version 1.10, in both languages, in one commit.**
   - The heading carries no version, or the same one as line 3.
   - The version line says what changed: section 2 brought to the code as of the commit.
2. **Section 2.** Rewrite the paragraph and the table's rows from the code and docs/profile, each statement naming its evidence. Keep the gaps that remain, for example the DMA boundary (no IOMMU in use yet) and stage VI not started. Do not present a plan as done (MC-12.3).
3. **Section 3:** lines 58 and 62 keep what was done then and say what changed since.
4. **README and constitution/README.md** name the roadmap without a version (000-KRN-0105).

## Acceptance criteria

- [ ] ROADMAP.md and ROADMAP_RU.md are both at 1.10. Their headings and version lines agree, and their sections 2 match each other and the code.
- [ ] No statement in section 2 contradicts sections 3 and 6 or the profile.

## Related

000-KRN-0105, [docs/profile](../docs/profile/README.md).
