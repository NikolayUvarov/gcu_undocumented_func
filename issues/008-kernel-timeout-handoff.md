# 008 — Handing control to userspace on timeout (not only on a key press)

**Type:** feature · **Priority:** low · **Status:** open
**Affects:** `kernel/src/main.rs`

## Discrepancy with the handoff

Handoff: the kernel "hands control to Userspace when the delay expires (or on a key press)". Code (`kernel/src/main.rs:29-32`): the switch happens only on scancode `0x39` (space); there is no timeout at all — without a key press the kernel spins the circle forever.

## Plan

1. Minimal: a `time` frame counter; when `time >= N` (e.g. 300 frames ≈ a few seconds with the `nop` delay) — call `app_entry`. Show a countdown (after [007](007-kernel-font-and-primitives.md)).
2. Proper: after [004](004-apic-idt-interrupts.md) — based on timer ticks, not frames.
3. React to any make-code key (not only space), ignoring break codes (`>= 0x80`), and flush the controller buffer before starting (`while status & 1 { in 0x60 }`).

## Acceptance criteria

- Without any key presses, the app's square appears after the configured time.
- Pressing any key makes the switch happen sooner.
