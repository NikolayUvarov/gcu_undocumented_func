# 800-LNK-0035 — The pointer over the link: the master's commands for moves, buttons and the wheel

**Type:** tool · **Owner:** `LNK` · **Priority:** P3 · **Status:** open (2026-10-31) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-12.1

**Split from** [800-LNK-0032](../issues-done/800-LNK-0032-keys-and-the-pointer-over-the-link.done), whose keys are done.

## Problem

- **What exists.** `link` posts the input words the master sends in an `INPUT` frame. A pointer word would be posted as the kernel takes it.
- **What is missing.** The master has no command that builds pointer words: `mindlink key`/`type` and `effector485`'s `key`/`type` give keys only.
- **Who needs it.** Checks in `wm` that need the mouse (APP's issues), on a machine no one is at.
- **Not yet known:** whether KRN's input capability lets `link` post pointer words as well as keys (asked of KRN on 2026-10-30, question 2).

## Plan

1. **Ask KRN again** whether the input capability covers pointer words, or what else is needed.
2. **`mindlink pointer move DX DY`, `pointer click [left|right|middle]` and `pointer wheel N`,** built as the PS/2 mouse decoder would give them, and the same as `effector485` operations.
3. **The QEMU `link` suite:** a pointer move and a click over the link reach `keys` (`pointer buttons=… dx=… dy=…`).

## Acceptance criteria

- [ ] Host tests of the pointer words.
- [ ] The QEMU `link` suite on x86 and aarch64: a move and a click over the link reach a program.
- [ ] On the MacBook Pro: a click over the cable in `wm`, with its effect on the screen.

## Related

[800-LNK-0032](../issues-done/800-LNK-0032-keys-and-the-pointer-over-the-link.done), [000-KRN-0108](000-KRN-0108-tests-on-a-real-machine-through-effector.md).
