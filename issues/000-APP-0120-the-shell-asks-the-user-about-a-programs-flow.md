# 000-APP-0120 — The shell puts a running program's question about a flow to the user

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** `netpolicy.wit` 1.3 (`questions`, `answer`), the network track's part of [000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md) · **Main task:** — · **Roadmap:** track C · **Constitution:** MC-3.1, MC-3.8, MC-3.11, MC-11.6

**Recorded by:** the tools track, 2026-10-31, from NET's request in `issues/requests-APP.md` and its message `APP/2026-10-31T0650Z-NET-a-flow-question-for-the-shell-to-put-to-the-user.md`.

## Problem

A program such as `get` reaches a host the user names only when it runs. Today a flow grant is only a line of the policy, written beforehand. The program may ask; only the user may answer, and only through the shell, which alone speaks to the user with the operator's authority (MC-3.11).

## Plan

- **NET's part** (`netpolicy.wit` 1.3, the two tracks' proposal):
  - a program's question through its grant: host, port and its own reason;
  - `questions()` and `answer(id, choice)` for the shell's client only, with `no`, `once`, `session` and `always`;
  - a question unanswered within 60 s refused and logged.
- **The shell's part:**
  - **Looking for questions:** the main loop asks `questions` at most every 500 ms.
  - **Where a question goes:** to the console the program was started from (`owners`), or to the shell's window for a program started there.
  - **The question:** the program's name as the loader knows it, the host, the port and its reason. The reason is shown as text the program gave, marked as the program's.
  - **The answer:** `N` (the default), `O`, `S` or `A`, read from the keyboard by the shell. The program's own output or input never answers it.
  - **A program in front** has the keys while it runs: the shell takes them for the question, as for the firmware's question, and gives them back.
  - **Then** `answer(id, choice)`. A question `netpolicy` refused meanwhile (its 60 s) is taken down with a line saying so.
- **The text** in `docs/tools` (EN, RU) and the shell's `HELP`.

## Acceptance criteria

- **QEMU, x86,** with the network track's test program:
  - asked, it is refused, then allowed once;
  - asked again, it is asked again;
  - `session` holds until the program ends;
  - `always` leaves the line in `netpolicy`.
- **The program cannot answer** its own question through its output or input.

## Related

[000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md), [351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done) (a question the shell already asks while a program waits).
