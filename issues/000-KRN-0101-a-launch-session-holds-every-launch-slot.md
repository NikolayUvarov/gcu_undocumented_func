# 000-KRN-0101 — A launch session holds every launch slot

**Type:** `loader` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made; the `bench` suite passes on x86; the gate left) · **Blocked by:** — · **Main task:** — · **Constitution:** MC-3.11, MC-3.3

## Problem

The tools track (requests-KRN.md, 2026-10-10, for [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done)): `loader`'s launch session kept at most 5 grants (`Session.grants: [(u8, usize); 5]`), and a sixth `grant` was refused with `limit`, while `LAUNCH_SLOTS` names 17 slots a launcher may fill.

- `console` started by `wm` may need six: the window, the user's files, system information, the camera, the shell's commands and, for `record -w`, the window to see. The `wm` suite's `record -w` lost its window lease to the limit.
- The tools track grants `SLOT_SHELL` last and lets a start go on without it, so the program runs without the shell's commands when the session is full. On the MacBook Pro `wm` itself now asks for six (211-APP-0070, on the tools branch), the battery last.

## Plan

- **The session holds** a grant for every launch slot and `SLOT_FIRMWARE` (`grant-firmware`): 18. With the loader's five standard clients and the debug mode's log, a spawn carries at most 24 of `SPAWN_GRANTS_MAX` (32), which a compile-time check holds.
- **A grant beyond it** still answers `limit`; a grant in a slot already lent replaces it, as before.

## Acceptance criteria

- [ ] **The `bench` suite's `check`, on x86 and aarch64:** a launcher lends an endpoint in each of the 17 launch slots in one session, and the program says it holds every one (`launch: every slot`).
- [ ] **The tools track** may then grant `SLOT_SHELL` (and the battery) in the order it likes; its own suites show `console` from `wm` with the shell's commands beside `record -w`'s window lease ([requests-APP.md](requests-APP.md) gets a note).

## Related

[requests-KRN.md](requests-KRN.md) (where the request was), [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done), `LAUNCH_SLOTS` in `common/abi.rs`.
