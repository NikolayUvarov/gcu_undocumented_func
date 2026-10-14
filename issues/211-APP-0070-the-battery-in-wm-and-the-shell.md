# 211-APP-0070 — The battery in `wm` (its top bar and its background) and in the shell

**Type:** tools · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (in `main`; the MacBook Pro's check left, 2026-10-14) · **Blocked by:** — · **Main task:** 211 · **Roadmap:** track G · **Constitution:** MC-3.11, MC-12.4

**Recorded by:** the tools track, 2026-10-12, numbering the kernel track's request in requests-APP.md ("The battery in `wm`'s top bar, and in the shell", for [211-PRT-0008](211-PRT-0008-the-battery-behind-the-embedded-controller.md)), taken from there. The maintainer: "The battery's state must also be shown in the indicator: on the mains, % or minutes left, switched", by a click; and on the background with a box, as the other lines: "can the battery's state go there too, on the desktop, with a checkbox like the others?"

## Problem

The battery service (`battery`, `idl/battery.wit`) reads the MacBook Pro's Smart Battery, and the shell holds its client in `SLOT_BATTERY` (211-KRN-0090). Nothing shows it:

- the shell neither lends the client for `REQUEST_BATTERY` nor has a command for it;
- `wm` shows no battery, neither in its top bar nor on its background.

A launch session keeps 5 grants (`loader`; requests-KRN.md, "A launch session holds as many grants as there are launch slots"). On the MacBook Pro `wm`'s launch already holds five: the window manager's client, the user's files, system information, the camera and the shell's commands. A sixth, the battery, is refused.

## Plan

- **The shell:**
  - lends `SLOT_BATTERY` for `REQUEST_BATTERY`, after its commands and only where the launch has room; `msh`'s word `battery`;
  - `shell.wit` 1.4 `battery`: a copy of its battery client, for a window manager whose launch had no room for it (the service only reads, and answers any client);
  - the command `battery`: the status in one line; `help`; a client of its commands may run it at once (`console` passes it on).
- **`wm`:**
  - asks for `REQUEST_BATTERY`; without the client after the launch, asks the shell (`[WM] BATTERY CLIENT FROM THE SHELL: …`); reads the status every 5 s; passes the client on to a program that asks for it;
  - the top bar's right end: ` ▮ 87% ` on the battery, ` AC 87% ` on the mains; a click switches to the minutes (` ▮ 2:41 ` left, ` AC 0:45 ` to full) and back; red below 10 % on the battery; no item without a battery;
  - the background: `show = … battery` and Settings' row `Battery` (off by default): the line `BAT 87% 2:41 LEFT`, `BAT 40% AC 0:45 TO FULL`, `BAT 100% AC`, `NO BATTERY`; on the analog face a third gauge.
- `docs/tools` (EN, RU), `docs/msh`, the IDL index.

## Acceptance criteria

- **Host tests** (`tests/wm_host.rs`): `show` reads and writes `battery`, off by default; the line in each state; the minutes; low; a line more on the digital face, a gauge more on the analog one; the top bar's item at the right end, the items leaving it room, a click switching it, red when low, none without a battery; Settings' row.
- **The shell suite** (x86 and aarch64): `battery` says there is no battery service; with an argument it says the command takes none.
- **The `wm` suite:** no battery service in QEMU: `wm` asks the shell and gets none, and the top bar has no item; Settings' `Battery` row ticks and unticks.
- **On the MacBook Pro** (the maintainer): the item shows the charge and follows the adapter being plugged in and out; a click shows the minutes; the background's line when ticked.

## Progress (2026-10-14)

In `main` (4500fd9) after the gate of 60653c1: the host tests, every x86 suite, every aarch64 suite on the i9 and 16 CPUs on the PCU.

- Done as planned: the shell's `battery` command and its lending (after its commands, where the launch has room), `shell.wit` 1.4 `battery`, `wm`'s top bar item with the click for the minutes, the background's line and Settings' row, the third gauge, `console` passing `battery` on.
- Checked in QEMU only, where no battery service runs: `battery` says so, `wm` asks the shell and gets none, the top bar has no item, Settings' row ticks and unticks. Nothing here was seen on a battery (MC-12.2).
- **Left:** the last criterion, on the MacBook Pro (the maintainer): the item follows the adapter, a click shows the minutes, the background's line when ticked.

## Related

[211-PRT-0008](211-PRT-0008-the-battery-behind-the-embedded-controller.md) (the service), 211-KRN-0090 (`SLOT_BATTERY`, `REQUEST_BATTERY`), [000-APP-0068](../issues-done/000-APP-0068-wms-background-seconds-and-sizes.done), [000-APP-0069](../issues-done/000-APP-0069-wms-background-analog-face-and-gauges.done), [requests-KRN.md](requests-KRN.md) (the launch session's grants).
