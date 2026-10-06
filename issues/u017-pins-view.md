# u017 — `pins`: the board's header on a screen, and `pins` in `wm` — done except the board run

**Type:** tools · **Owner:** tools track · **Priority:** P3 · **Status:** open (done except the board run) · **Blocked by:** — · **Roadmap:** track H · **Constitution:** MC-3.3, MC-3.11, MC-10.2

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

## Progress (2026-10-06)

- **Done — `pinmap`** (`pins/src/bin/pinmap.rs`; the view in `pins/src/view.rs`), a separate program rather than `pins --view`: a program is a console program or has a screen, and that is decided before it runs.
  - The header in two columns as on the board, odd positions left; without a board file, the pins in order.
  - Each pin's function by name and its level, refreshed every 100 ms; reserved pins dim, outputs and alternates coloured.
  - Enter: the pin's functions (the active one marked `*`), Enter picks one. `w` toggles an output, `p` cycles the pull.
  - The first change of a session asks ("Change a pin? … This drives the board's hardware"); a refusal names its reason.
  - Without a gpio client it says so on its screen.
- **Done — through `wm` and `console`:** both ask for `REQUEST_GPIO` and pass the client on to a program that asks for it. `wm`'s System menu lists `pinmap`; `pins` runs in a `console` window.
- **Done — tests:**
  - `tests/pins_host.rs`: the Raspberry Pi 4's header (positions 1–40, power and ground, pin 14 at position 8 with `ALT0 TXD0`), a change after one confirmation, then none asked; a cancelled change, a reserved pin refused, power or ground.
  - QEMU `shell` suite on x86 and aarch64: `pinmap` says there is no pin controller and Esc ends it.
- **Not done:** pin masks per client (they wait for the service, 207); the mouse (keys only).
- **Open — the board run** (with 205).

## Acceptance criteria

- Host test: the layout of the Raspberry Pi 4's header (positions 1–40, pin numbers, functions) and the confirmation.
- QEMU: `wm`'s menu entry for `pins` without a controller says so, and `pins --view` says so on its screen.
- On a Raspberry Pi 4 (with 205): the view shows UART0 on positions 8 and 10, and a click changes pin 17.

## Related

[u015](u015-pins.md), [207](207-gpio-service.md), [205](205-aarch64-boards.md), [088](../issues-done/088-text-window-manager.done).
