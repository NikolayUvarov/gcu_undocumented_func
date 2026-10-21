# 850-KRN-0146 — A refusal names the clause it enforces

**Type:** `init`, `loader`, kernel log lines · **Owner:** kernel session · **Priority:** P3 · **Status:** open (2026-10-21) · **Blocked by:** — · **Main task:** [850](850-the-cultural-code.md) (the kernel track's part) · **Constitution:** MC-10.2, MC-12.3

## Problem

The system's voice in its logs is precise but anonymous. For example:
- `init` refuses a mode change or a reboot to a client without the badge;
- the loader refuses a program;
- the kernel refuses a capability operation.

None of these says which requirement it enforces. A reader of a log, or of a screenshot, cannot connect the refusal to the Constitution, though every requirement has a stable identifier.

## Plan

- **The line.** A refusal that enforces a constitutional requirement ends with its clause, for example `[INIT] MODE CHANGE REFUSED: PID 42 (BADGE 2) MAY NOT TURN MAINTENANCE ON (MC-12.6)`.
- **The first set,** the kernel track's own components:
  - `init`'s refusals (`reboot`, `set-mode`, `stop` of a service that may not stop);
  - the loader's refused programs (a manifest or signature, MC-9.x);
  - the kernel's refused rights (`ERR_RIGHTS` in its log lines, MC-3.x).
- **Other tracks** get a short rule in CONTRIBUTING.md and adopt it as they touch their refusals. Not one large change.
- **Only where the link is exact.** A refusal that enforces no clause names none.

## Acceptance criteria

- [ ] The listed refusals name their clause, and the suites that read those lines still pass.
- [ ] CONTRIBUTING.md has the rule.

## Related

[850](850-the-cultural-code.md), [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md) (the mode refusals).
