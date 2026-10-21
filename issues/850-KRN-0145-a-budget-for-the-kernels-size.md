# 850-KRN-0145 — A budget for the kernel's size, checked in CI

**Type:** CI, scripts · **Owner:** kernel session · **Priority:** P3 · **Status:** open (2026-10-21) · **Blocked by:** — · **Main task:** [850](850-the-cultural-code.md) (the kernel track's part) · **Constitution:** MC-1.1–1.3 (a minimal trusted kernel), MC-12.3

## Problem

- **The size today.** The kernel is 7,571 lines of Rust in `kernel/src`, two architectures included (2026-10-21, `main` 41a5117). That is a fact worth stating, and a property worth keeping (Article 1).
- **Nothing keeps it.** A mechanism that could live in user space can grow inside the kernel one commit at a time, and nothing would say so.

## Plan

- **The check.** `scripts/kernel_budget.py` counts the lines of `kernel/src`, without blank lines and comments, and the lines of each architecture apart.
- **The budget.** A file states it, with the date and the commit it was measured at. The host tests fail when the count is above the budget.
- **Raising the budget is a decision in its own commit,** which names the mechanism that needs the room and why it cannot live in user space (MC-1.2).
- **The report** goes into the host tests' output, so that the number stated in `LORE` (850-PHI-0002) can always be checked.

## Acceptance criteria

- [ ] The check runs in `scripts/host_tests.sh` and fails above the budget.
- [ ] The budget file states the measured size and the commit.

## Related

[850](850-the-cultural-code.md), [850-PHI-0002](850-PHI-0002-the-canons-maxims-in-lore-1-6.md), [docs/profile/tcb.md](../docs/profile/tcb.md).
