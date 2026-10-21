# 850 — The cultural code: a manifesto, the canon's maxims and a lexicon that the code bears out

**Type:** main task (philosophy, documents) · **Owner:** `PHI` · **Priority:** P2 · **Status:** open (2026-10-21) · **Blocked by:** the maintainer's decisions below for the manifesto's voice · **Roadmap:** — (the maintainer's request, 2026-10-21) · **Constitution:** MC-12.3, MC-12.5; the Preamble

## Problem

The maintainer proposed on 2026-10-21 to use the structure of an "iconic IP guide" (the bibles that hold together worlds such as Star Wars or Cyberpunk) so that MIND Core can be understood and shared beyond the people who build it.
- **What exists.** The Constitution v1.6 is the law: requirements, numbered `MC-<article>.<clause>`, deliberately sober ("novelty in itself is not a criterion of quality").
  - `constitution/lore/LORE.*.md` holds the five disciplines (ontology, epistemology, ethics, praxeology, aesthetics). It is still edition 1.5 and names the Constitution 1.5 and RFC 0.3.
  - The README's "Why MIND Core" is the only outward story.
- **What is missing:**
  - a one-page manifesto;
  - short maxims a reader can quote;
  - a lexicon that matches the code's own names;
  - one rule for the system's voice in its logs and errors.
- **The trap.** In an operating system the philosophy must compile into code. A text that promises more than the code does is found out by the first careful reader, and the Constitution forbids presenting intentions as guarantees (MC-12.3).

## Plan

1. **The manifesto** ([850-PHI-0001](850-PHI-0001-the-manifesto.md)): one page, English and Russian, non-normative, outside the Constitution's articles.
2. **The canon's maxims** ([850-PHI-0002](850-PHI-0002-the-canons-maxims-in-lore-1-6.md)): `LORE` 1.6, aligned with the Constitution 1.6. Each discipline gets one maxim, with what enforces it today and what does not yet.
3. **Philosophy compiled into checks, the kernel track's part:**
   - [850-KRN-0145](850-KRN-0145-a-budget-for-the-kernels-size.md): a budget for the kernel's size, checked in CI;
   - [850-KRN-0146](850-KRN-0146-refusals-name-the-clause.md): refusals name the clause they enforce.
4. **A lexicon** (a later task): the entities' names, each mapped to an identifier in the code. A word the code does not use is not introduced.
5. **The sensory code** (later, with `APP`): the first boot, the system's voice, one visual anchor. These are designs for the tools track, not promises in the manifesto.
6. **The Constitution itself.** It changes only by a new edition with the contents MC-12.5 asks, in both languages, and only if the analysis finds a requirement to add. Rhetoric does not go into the articles.

## Questions to settle (the maintainer)

- **Who the hero is.** The Constitution builds a substrate for an autonomous Mind. It requires no universal human administrator and gives the Mind no unbounded bypass of control (MC-12.6). Is the protagonist the person who grants mandates to Minds, the Mind itself, or both?
- **What the adversary is.** The proposal is ambient authority and blind trust, which the code does fight: mandates, drivers outside the kernel, a signed boot manifest, updates on trial with a fallback. The alternative is corporations or other systems, which it does not fight. MIND Core is itself built and tested on Linux.
- **Iain M. Banks's names.** The Culture, Minds, Marain and the ship classes belong to the author's estate. This is a legal question: [issues-human](../issues-human/README.md), section 11.

## Acceptance criteria

- [ ] 850-PHI-0001 and 850-PHI-0002 are done, in both languages.
- [ ] Every claim of the manifesto and of the maxims links to its evidence (a suite, a profile row) or is marked as a goal.
- [ ] The questions above are answered and recorded here.

## Related

[Constitution v1.6](../constitution/EN/MIND_CORE_Constitution_v1.6.md), [LORE](../constitution/lore/LORE.EN.md), [RFC 001 Marain](../constitution/EN/RFC_001_Marain_v0.4.md), [docs/profile](../docs/profile/README.md), README "Why MIND Core".
