# 154 — Push-to-talk routing

**Type:** kernel · **Owner:** kernel track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track G (voice V2+) · **Constitution:** MC-3.3, MC-10.2

## Problem

Input events go to the focused task; only the attention key (Ctrl+Z) reaches the focus owner whatever has the focus. Voice control (079) listens on F12 only while the shell has the focus; a push-to-talk key should work over any program without letting that program — or the voice program — read other keys.

## Plan

- `INPUT_LISTEN` (process control): the focus owner registers one key code and an endpoint; presses and releases of that key are sent there as `MSG_FLAG_INPUT` notices and removed from the focused task's stream; all other keys are unchanged. One registration per system, replaced by the next call, dropped when the endpoint dies.
- The shell registers F12 with the endpoint of its voice program (079).

## Acceptance criteria

- QEMU `keys` suite: with `view` in focus, F12 reaches the registered endpoint and not `view`; other keys reach `view`; a task without process control gets `ERR_RIGHTS`.

## Related

[079](079-voice-control-in-the-shell.md), [docs/voice](../docs/voice/README.md).
