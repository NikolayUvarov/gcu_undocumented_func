# u015 — `pins`: the pins of an ARM board — list, functions, signals

**Type:** tools · **Owner:** tools track · **Priority:** P2 · **Status:** open (done except the board run) · **Blocked by:** [205](205-aarch64-boards.md) (a board to run on; [207](207-gpio-service.md) gives the service) · **Roadmap:** track H (with track F for physical control) · **Constitution:** MC-3.3, MC-10.2

Requested by the porting track for the user in `tools-track-reports.md` (2026-10-06), numbered here.

## Problem

Requested by the user (2026-10-06): a tool that, on an ARM board with a pin controller, shows the available pins and their functions, and for a multiplexed pin lists all its functions with the active one marked; and lets the user drive the signals on them. Documentation it needs (which signal each function number is on which SoC, the board's header) lives apart in [hwdocs/](../hwdocs/README.md), not in the boot image.

## Plan

- Talks only to the `gpio` service ([207](207-gpio-service.md); the porting track provides it with [206](../issues-done/206-pin-controllers-from-firmware.done)). Without `gpio` (x86, a board without a known controller) it says that there is no pin controller and exits.
- `pins` (list): controller, pin, header position (from the board file), active function by name (`TXD0`, `GPIO out`), level, pull, reserved mark. `pins <n>`: every function of pin n, the active one marked (`*`). Names come from `/hwdocs/socs/<soc>.pins` and `/hwdocs/boards/<board>.board` when present; without them, numbers (`ALT0`…`ALT5`) and a note that the tables are missing.
- Changing: `pins set <n> out|in|alt<k>`, `pins write <n> 0|1`, `pins pull <n> up|down|none`, `pins watch <n…>` (levels refreshed). Changes need the control badge; the tool says when it is refused and why (reserved, not granted).
- A full-screen and window view (like `load`, `memmap`): the header drawn as two columns of pins coloured by function; click or Enter on a pin shows its functions and lets the operator change it, with a confirmation before the first change of a session.
- `wm`'s menu: under System.

## Progress (2026-10-06)

- **Done — the program** `pins` (`pins/src/main.rs`; the logic in `pins/src/tool.rs`): a console program asking for `REQUEST_CONSOLE | REQUEST_GPIO`.
  - **Commands:** `pins`, `pins <n>`, `pins set <n> in|out|alt<k>`, `pins write <n> 0|1`, `pins pull <n> up|down|none`, `pins watch <n>... [-t seconds]`, and `-c <controller>`.
  - **Refusals say why:** reserved by the board, no control client, not an output, no pulls on a PL061, no such pin.
  - **Exit codes:** 1 without a controller or service, 2 when refused or used wrongly.
  - **No `/hwdocs`:** function numbers.
  - **No gpio client:** it says that the service runs only where the firmware names a known controller and that the shell lends it.
- **Done — tests:**
  - `tests/pins_host.rs`: a model service built like `gpio/src/main.rs` from `mind::gpio`'s register models and the hwdocs tables.
  - QEMU `shell` suite on x86 and aarch64: no gpio client.
- **Done — docs:** README, docs/tools §4.11 (EN and RU).
- **Split off — [u017](u017-pins-view.md):** the full-screen and window view (the header as two columns of pins) and the place in `wm`'s menu. Under `wm` and `console`, `pins` has no client yet, because only the shell lends one.
- **Fixed with 158:** the loader refused `SLOT_GPIO` in a launch session (its list of slots a launcher may fill lacked it), so on a board the shell could not have lent the gpio client and `pins` would not have started. QEMU cannot show it (no `gpio`); the same list now takes `SLOT_CAMERA`, which the QEMU camera test exercises.
- **Open — the board run** (with 205).

## Acceptance criteria

The criteria were amended on 2026-10-06. The reason: issue 206 found that QEMU `virt` with UEFI and ACPI has no pin controller (its PL061 exists only with `acpi=off`, which this kernel cannot boot). So the QEMU end-to-end part could not be met, and it moved to the board run, as it did for 207.

- **Host test** against the register models of `mind::gpio` and the repository's hwdocs tables:
  - the list, and the per-pin view (pin 14: ALT0 `TXD0` active, ALT1–ALT5 listed);
  - `set`, `write` and `pull` read back;
  - a reserved pin refused, and a client without the control badge refused;
  - function numbers without `/hwdocs`;
  - a PL061 without pulls.
- **QEMU** (x86 and aarch64): `pins` reports that it has no gpio client.
- **On a Raspberry Pi 4** (with 205): the 58 pins are listed with UART0 active on 14/15, and `pins set 17 out` then `pins write 17 1` lights an LED on header position 11.

## Related

[206](../issues-done/206-pin-controllers-from-firmware.done), [207](207-gpio-service.md), [hwdocs/](../hwdocs/README.md).
