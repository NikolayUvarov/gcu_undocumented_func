# 800-KRN-0191 — With `dev`, `link` holds the input capability, so that keys and the pointer go over the cable

**Type:** `init` · **Owner:** `KRN` · **Priority:** P1 · **Status:** in progress (in `main` 3fbdd30e; the safe-start check, aarch64 and LNK's check left) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) (the link track's), for 800-LNK-0032 and [000-KRN-0108](000-KRN-0108-tests-on-a-real-machine-through-effector.md) · **Roadmap:** track C · **Constitution:** MC-3.1, MC-3.11, MC-10.2, MC-12.3

**Asked by:** the link track (`LNK`) on 2026-10-30, for 800-LNK-0032, in `KRN/2026-10-30T1222Z-LNK-a-request-the-input-capability-for-link-in-dev.md` on `internal-dev-messages`. The maintainer wants key presses over the cable (2026-10-30). 000-KRN-0108's rig stays for the firmware's menus, the boot and a hung machine.

## Problem

- **What `link` already does.** It carries the link's `INPUT` frames: input words as `common/abi.rs` `input_event` builds them, up to 64 a frame.
- **What it lacks.** Nothing lets it post them. `SYSCALL_INPUT_EVENT` is open only to a holder of the input capability, which the keyboard drivers hold, and the shell for the serial line.

## Plan

- **`init`'s plan for `link`:** with `dev` in `link.conf`, and only then, the input privilege (`CAP_KIND_INPUT`) in `SLOT_INPUT`, beside the other developer grants (800-KRN-0139).
  - Not in a safe start: the person who asked for a safe start is at the machine.
  - Nothing else comes with it.
  - `init`'s developer-mode line names it.
- **Why the capability itself, not a call to the shell:**
  - In `dev`, `link` already holds the shell's commands (800-KRN-0148), a launch-only policy client and the reboot. Posting keys gives it no power it lacks.
  - The kernel's route is the one the keyboard drivers use: focus, listened keys (`INPUT_LISTEN`) and the attention key work alike for every source.
- **The pointer:** the same capability posts the kernel's pointer words (`KEY_POINTER`). The kernel gives them only to the task with the focus that asked for the pointer (`INPUT_POINTER`), as for a mouse.
- **No ABI change:** the call, the capability and the event word exist.

## Acceptance criteria

- **QEMU, the `link` suite, x86 and aarch64:**
  - with `dev`, `stat caps` of `link` shows the input privilege in `SLOT_INPUT`;
  - without `dev`, and in a safe start with `dev`, it does not;
  - LNK's check: keys posted over the cable reach the shell (800-LNK-0032).
- **A run on the MacBook Pro:** LNK's, with its `mindlink type`.

## Landed (2026-10-30)

- **In round 54** (`main` 3fbdd30e), from `claude/youthful-mendel-mf1soy` 9e884aae:
  - with `dev` in `link.conf`, `init` puts the input privilege (`CAP_KIND_INPUT`) in `link`'s `SLOT_INPUT`, never in a safe start;
  - its developer-mode line names `INPUT`, or `NO INPUT IN A SAFE START`.
- **Checked in the gate of 485a7129** (`x86: services, storage, audio`, QEMU under TCG): the `link` suite sees `SLOT=8 GEN=0 KIND=6` in `link`'s capabilities with `dev`.
- **Round 53's lesson.** LNK's key check passed `alt-0` and `shift-a` through, but not `ctrl-alt-f6`: the shell listens for Ctrl+Alt+F1…F6, and the kernel gives a listened key to its listener from any source, as for the keyboard. So the routing is as planned, and LNK's check takes another chord. LNK's request was returned for that (`LANDING/2026-10-30T1422Z-KRN-round-04e043f5.md`).
- **Left:**
  - the check that a safe start with `dev` grants no input privilege;
  - the same checks on aarch64;
  - LNK's check that keys over the cable reach the shell (800-LNK-0032).

## Related

[800-KRN-0139](800-KRN-0139-the-links-developer-mode.md), [800-KRN-0148](800-KRN-0148-a-shell-client-for-link-in-developer-mode.md), [800-KRN-0173](800-KRN-0173-a-firmware-scope-for-link-in-developer-mode.md), [000-KRN-0108](000-KRN-0108-tests-on-a-real-machine-through-effector.md), 800-LNK-0032.
