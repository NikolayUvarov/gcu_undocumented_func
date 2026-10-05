# 160 — A program in front hands the focus to the program it starts

**Type:** kernel + loader · **Owner:** kernel track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Roadmap:** track G · **Constitution:** MC-3.3, MC-3.11, MC-10.2

## Problem

Reported by the user: a program started from `fm` "does not work and goes to the background". On a screen of its own, `fm` starts programs through the loader (`loader.wit` `begin` / `commit`); the program gets a screen but stays behind `fm`, because only a task with process control (the shell) may call `FOCUS`. The user has to press Ctrl+Z and type `FG <pid>` in the shell, and when the program ends the shell, not `fm`, comes back.

Under `wm` this no longer happens: `fm` in a window lends its broker client and the program opens a window of its own in front ([099](../issues-done/099-fm-starts-programs-in-windows.done)). On a full screen there is no such way.

## Plan

- **Handing over the focus.** A task that has the focus may give it to a task started on its behalf, and only to that one:
  - `loader.wit` `commit` gets a `foreground` flag. The loader passes it to `SPAWN` as `SPAWN_FOREGROUND` with the session owner's PID;
  - the kernel honours it only if that PID has the focus at that moment, so a program in the background cannot take the screen;
  - the new task's input goes to it at once, as after `FG`.
- **Coming back.** When a task that received the focus this way ends, the focus returns to the task that handed it over, if it still runs; otherwise to the shell, as now. Ctrl+Z still goes to the shell.
- **Programs:** `fm` (and `wm` for a program that is not in a window) commits with `foreground` and says `Started top (PID n)`; the shell keeps `FOCUS`.

## Acceptance criteria

- QEMU:
  - `fm` on its screen starts `top` with Enter: `top` is in front and gets the keys; Esc in `top` brings `fm` back with its panels;
  - a program in the background that commits with `foreground` is refused, and the focus stays where it was;
  - Ctrl+Z from the started program still returns to the shell.
- The kernel's focus rules are listed in docs/api.

## Related

[099](../issues-done/099-fm-starts-programs-in-windows.done), [088](../issues-done/088-text-window-manager.done), [155](155-virtual-consoles.md), [154](../issues-done/154-push-to-talk-routing.done).
