# u017 — `pins`: the board's header on a screen, and `pins` in `wm`

**Type:** tools · **Owner:** tools track · **Priority:** P3 · **Status:** open · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-3.3, MC-3.11, MC-10.2

Split off from [u015](u015-pins.md), whose console commands are done.

## Problem

`pins` lists pins as text. The user asked for a view of the board: the header as it is on the board, the function of each pin at a glance, and changes made by pointing at a pin. And `pins` cannot run from `wm`'s menu yet. A program `wm` or `console` starts gets only what they hold, and only the shell holds the `gpio` control client (`SLOT_GPIO`).

## Plan

- **`pins --view`, a screen program like `load` and `memmap` (a pixel window in `wm`):**
  - the header drawn as two columns of pins, coloured by function (input, output, each alternate, power and ground from the board file), with the level shown;
  - a click or Enter on a pin shows its functions;
  - a change asks for confirmation before the first one of a session;
  - the levels are refreshed every 100 ms;
  - without a board file, a list.
- **The client under `wm`:** `wm` asks for `REQUEST_GPIO` and passes the client on, as it does for the files and system information. `wm`'s menu lists `pins` under System. Without the client, the entry says why.
- **Pin masks** per client (207's open "pins 17–27 only"), if the service gets them first.

## Acceptance criteria

- Host test: the layout of the Raspberry Pi 4's header (positions 1–40, pin numbers, functions) and the confirmation.
- QEMU: `wm`'s menu entry for `pins` without a controller says so, and `pins --view` says so on its screen.
- On a Raspberry Pi 4 (with 205): the view shows UART0 on positions 8 and 10, and a click changes pin 17.

## Related

[u015](u015-pins.md), [207](207-gpio-service.md), [205](205-aarch64-boards.md), [088](../issues-done/088-text-window-manager.done).
