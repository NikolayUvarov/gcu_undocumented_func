# Ready again: `run -d` of a program that only reads, `sshd`'s badge, the clipboard's last checks; three issues closed

**Ready:** `claude/wizardly-franklin-kec1a9` 171d37ecfaf6bed9989ca5fc6e5e0594532572aa

It contains `main` 7c1c68f563 (round 128). It replaces `LANDING/2026-11-13T0852Z-APP-ready-run-d-read-only-sshds-badge-and-two-closed.md`, which was not answered and is in no candidate.

- **Closes** 112-APP-0136, 312-APP-0134 and 000-APP-0032 (moved to `issues-done/`); records 000-APP-0137 (the `tools` suite on aarch64, which the gate does not run).
- **As 0852Z said:** the shell takes `mind::process::BADGE_SHELL_REMOTE`; `run -d` of a program that only reads through `domain-client-with(id, BADGE_GET)`; `clipboard` logs no line for each program named in front.
- **New since 0852Z:**
  - `tests/clip_stub`, a test-only console program built by `scripts/x86_fixtures.sh` and `scripts/build_aarch64.sh` as the `secret` and `storewrite` stubs are; `clipboard_check` runs it in front (it pastes), in the background (refused, `Behind`) and as a copy that asks for no clipboard (no client);
  - `monitors_check` reads `top`'s clock row with the applications alone and selects its first row before the PID-sorted table, pages `memmap`'s list for the kernel arena, and expects the task window at the architecture's base: on aarch64 the tasks (30, with `clipboard`) and the boot images outgrew the screen. x86 is unchanged by these.
- **Tested** (QEMU, 4 CPUs, in this container):
  - x86: `tools` (11, the clipboard check whole) on f98e7a17 and with the stub on 1da784a9; `tls` (13), `store` (33) on fd6f7bbb;
  - aarch64: `store` (38) on fd6f7bbb; `tools` now passes `top` and `memmap` and fails at `hw`'s x86 expectations (`+NX`, the IDE controller, `ps2_kbd`), as before these changes and outside the gate: 000-APP-0137;
  - host tests: `shell_host` (38), `clipboard_host` (4), `fm_host` (23); the stub builds on both.
- **Merges** with `main` as it is; no other request waits.
