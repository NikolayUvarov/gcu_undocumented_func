# u015 — `pins`: the pins of an ARM board — list, functions, signals

**Type:** tools · **Owner:** tools track · **Priority:** P2 · **Status:** open · **Blocked by:** [207](207-gpio-service.md) (the `gpio` service; it needs [206](206-pin-controllers-from-firmware.md)) · **Roadmap:** track H (with track F for physical control) · **Constitution:** MC-3.3, MC-10.2

Requested by the porting track for the user in `tools-track-reports.md` (2026-10-06), numbered here.

## Problem

Requested by the user (2026-10-06): a tool that, on an ARM board with a pin controller, shows the available pins and their functions, and for a multiplexed pin lists all its functions with the active one marked; and lets the user drive the signals on them. Documentation it needs (which signal each function number is on which SoC, the board's header) lives apart in [hwdocs/](../hwdocs/README.md), not in the boot image.

## Plan

- Talks only to the `gpio` service ([207](207-gpio-service.md); the porting track provides it with [206](206-pin-controllers-from-firmware.md)). Without `gpio` (x86, a board without a known controller) it says that there is no pin controller and exits.
- `pins` (list): controller, pin, header position (from the board file), active function by name (`TXD0`, `GPIO out`), level, pull, reserved mark. `pins <n>`: every function of pin n, the active one marked (`*`). Names come from `/hwdocs/socs/<soc>.pins` and `/hwdocs/boards/<board>.board` when present; without them, numbers (`ALT0`…`ALT5`) and a note that the tables are missing.
- Changing: `pins set <n> out|in|alt<k>`, `pins write <n> 0|1`, `pins pull <n> up|down|none`, `pins watch <n…>` (levels refreshed). Changes need the control badge; the tool says when it is refused and why (reserved, not granted).
- A full-screen and window view (like `load`, `memmap`): the header drawn as two columns of pins coloured by function; click or Enter on a pin shows its functions and lets the operator change it, with a confirmation before the first change of a session.
- `wm`'s menu: under System.

## Acceptance criteria

On QEMU `virt` (aarch64 suite) `pins` lists the PL061's pins, `pins 3` shows input/output with the active one marked, `pins set 3 out` and `pins write 3 1` change the level `pins` reads back, a reserved pin is refused; without `/hwdocs` it shows function numbers; a host test checks the list and the per-pin view against a recorded `gpio` answer and the BCM2711 tables (pin 14: ALT0 `TXD0` active, ALT1–ALT5 listed). On x86 it reports no pin controller.

## Related

[206](206-pin-controllers-from-firmware.md), [207](207-gpio-service.md), [hwdocs/](../hwdocs/README.md).
