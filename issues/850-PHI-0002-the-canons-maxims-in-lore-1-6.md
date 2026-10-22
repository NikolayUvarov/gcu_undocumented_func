# 850-PHI-0002 — The canon's maxims: `LORE` 1.6, one maxim per discipline, with what enforces it today

**Type:** document (English and Russian) · **Owner:** `PHI` · **Priority:** P2 · **Status:** open (2026-10-21) · **Blocked by:** — · **Main task:** [850](850-the-cultural-code.md) · **Constitution:** MC-12.3; the Preamble; Articles 1–4, 6, 10

## Problem

- **`LORE` 1.5 is out of date.** `constitution/lore/LORE.*.md` sets out the five disciplines in edition 1.5, but it still names the Constitution 1.5 and RFC 0.3.
- **Nothing in it is short enough to quote.**
- **Some formulations proposed on 2026-10-21 are false today:**
  - "nothing exists but the immutable (CAS) or the isolated (actors)": most data lives on mutable FAT volumes, beside the content-addressed block store;
  - "no byte leaves for the network without the user's cryptographically confirmed intent": flows are granted by `netpolicy` from its policy file;
  - "Marain cannot express a vulnerability": there is no compiler, and RFC 001 itself says compilation confirms neither authority nor safety.

## Plan

`LORE` 1.6, aligned with the Constitution 1.6, in English and Russian. For each discipline:
- **A maxim** that a reader can quote. Candidates:
  - ontology: "No shared state: every service is a task of its own; memory is shared only when lent, explicitly";
  - epistemology: "Without a mandate, the object does not exist for you" (endpoints have no global names, `common/abi.rs`);
  - ethics: "A service's death is its own event, not the end of the world" (restart budgets, quarantine, 000-KRN-0075);
  - praxeology: "No telepathy through global state: only an explicit, typed dialogue" (IPC and `idl/*.wit`);
  - aesthetics: "The kernel is 7,600 lines of Rust for two architectures; everything else, drivers included, is outside it" (a number, measured, held by [850-KRN-0145](../issues-done/850-KRN-0145-a-budget-for-the-kernels-size.done)).
- **Two columns:**
  - "enforced today", with the clause and its evidence: a suite, a profile row;
  - "not yet", what the code does not do.

  This turns "show me the test" into the form of the canon itself.
- **The links** name the current documents.

## Acceptance criteria

- [ ] `LORE.EN.md` and `LORE.RU.md` are edition 1.6, changed in one commit, naming the Constitution 1.6 and RFC 0.4.
- [ ] Every maxim has both columns. Every "enforced today" names a clause and its evidence.
- [ ] No maxim states as true what the code does not do.

## Related

[850](850-the-cultural-code.md), [850-PHI-0001](850-PHI-0001-the-manifesto.md), [docs/profile](../docs/profile/README.md).
