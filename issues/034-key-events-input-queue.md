# 034 — Key events in the per-task input queue (tools F4, kernel part)

**Type:** kernel · **Owner:** kernel track (coordinates `kernel/src/scheduler.rs`, `common/abi.rs`) · **Priority:** P1 · **Status:** open · **Roadmap:** track G, T0 · **Constitution:** MC-1.1

## Problem

A task's input queue holds bytes: raw PS/2 make codes and raw UART bytes share one stream, there are no key releases, modifiers or extended keys. Tools (editor, file manager) need arrows, function keys and modifiers.

## Plan

- One event word `KeyEvent { key, mods, ch, pressed }` (layout in `common/abi.rs`, shared with the tools track).
- The per-task queue holds 64 events; `INPUT_EVENT` takes an event word; new `READ_INPUT` returns one (0 if none); `READ_KEY` stays for existing programs (the low byte of `ch`).
- The attention flag (Ctrl+Z) is unchanged. No decoding or layouts in the kernel (they are the tools track's `ps2_kbd` and shell work, issue 043).

## Acceptance criteria

- Host test of the event packing; `isolation` case: an application reads events injected by the shell; `normal` suite unchanged.
- Profile and README updated.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F4; [043](043-keyboard-decoding-and-line-editing.md).
