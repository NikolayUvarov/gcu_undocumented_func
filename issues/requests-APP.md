# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

Numbered on the tools branch (2026-10-09): clocks and the RTC (000-APP-0012), `log:` and `efivar` in the tools (211-APP-0013), full screen and the list of windows in `wm` (211-APP-0014), `update` (351-APP-0029), `wifi` (550-APP-0033), the audit's A07 and A08 (175-APP-0035, 175-APP-0036), the marked window (211-APP-0037), the message a program that ends at once leaves in its window (211-APP-0039), the shell's commands in `wm` (211-APP-0040), Russian speech on the MacBook Pro with the kernel track's measured note on its hiss and clicks (252-APP-0041) and `date set` (211-APP-0042). Numbered on 2026-10-10: the camera from `wm` and the shell in a window, as the camera from `wm` and `console` (158-APP-0043), the shell's own window (211-APP-0040) and `console` joined to the shell (211-APP-0044, waiting for `SLOT_SHELL` from `KRN`); the shell's window only when asked (211-APP-0045) and a window back to its content's size (211-APP-0046); later that day the menu's tests category (176-APP-0051), voice control as a service (000-APP-0052), what `hear` heard (000-APP-0053), the help keys in the shell's commands (000-APP-0054), the date and time from `wm`'s Settings (000-APP-0055), every menu entry reacting (000-APP-0056) and the machine's memory in `top` and `free` (211-APP-0057); then `cpus` naming AVX-512 and AMX (174-APP-0058). The requests below wait.

## `svc boot`, `enable`, `disable`, `after`, `reset`: which services start at boot (173)

**Recorded by:** the kernel track (KRN), 2026-10-09, for main task [173](173-boot-services-configuration.md) at the maintainer's request.

### Problem

The maintainer wants a tool to turn boot services on and off and to order them when needed. init will read `data/services.txt` ([173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md)) and report the plan through `init.wit` `boot-plan`. Nothing lets the user change the file but editing it by hand.

### Plan (a proposal; the tools track decides)

- `svc boot`: the plan from init (order, enabled, essential, off and why) and how the last boot went.
- `svc enable <service>`, `svc disable <service>`, `svc after <service> <other>` edit `data/services.txt` through the shell's file client and say that the change applies at the next boot; `svc reset` removes the file.
- What init would refuse is refused here first: essential services, unknown names, an order against a dependency.
- The help screen and `docs/tools` (EN, RU) describe it.

### Acceptance criteria

The `tools` suite runs `svc disable tts`, reboots, and finds `tts` off in `svc boot`. `svc disable logd` is refused.

## Horizontal scrolling from a trackpad (211-DRV-0018)

**Recorded by:** the kernel track (KRN), 2026-10-09, for [211-DRV-0018](211-DRV-0018-macbook-trackpad-gestures.md), at the maintainer's request.

### Problem

`usb_hid` now reads the MacBook Pro trackpad's fingers. A three-finger swipe up or down turns the wheel, which `wm` and the programs already use. A swipe left or right turns a horizontal wheel, which nothing reads yet:
- relative pointer events carry it in bits 34–37 (`pointer_scroll`, `pointer_across` in `common/abi.rs`, zero in every other event);
- `mind::input::Pointer` has no field for it;
- `wm` passes only `wheel` to windows (`desk.rs`, `to_window`);
- the window protocol has no field for it.

### Plan (a proposal; the tools track decides)

- `mind::input::Pointer` gains `across` (from `pointer_across`).
- `wm` passes it to the window under the pointer, as the wheel. The window event carries it, in a new minor version of `idl/window.wit` if the event's layout changes.
- `view`, `edit`, `fm` and the text windows scroll sideways by it where their content is wider than the window.

### Acceptance criteria

A host test of `wm`'s routing passes a horizontal step to the window under the pointer, and `view` scrolls a wide image sideways by it. On the MacBook Pro a three-finger swipe left or right moves a wide image in `view`.

## Note: the shell's text for `loader.wit` 1.7 `unreadable` (211-KRN-0050)

The kernel track added `unreadable` to the loader's errors. The shell's match on loader errors is exhaustive, so the interface change took the shell's mapping with it: `ERR_IO` → `CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)` in `shell/src/main.rs`. The tools track may word it otherwise. `wm`, `fm` and `console` print loader errors with `{:?}` and show `Unreadable`.

## Note: the camera is lent without a question (158; the maintainer's rule, 2026-10-09)

The maintainer ruled that a program the user starts gets the devices it is for without a question (CONTRIBUTING.md, "No question about a tool's own purpose"). At that instruction the kernel session removed the shell's `ASKS FOR THE CAMERA. ALLOW?` and changed `shell/src/main.rs`, `docs/tools` (EN, RU), `camera`'s help and the `video` suite's camera check. The tools track may revise the wording. Questions stay where an action goes beyond the tool's purpose: the firmware's boot settings, the network policy.

## `files` reads the kernel from the slot that booted (351-UPD-0016)

**Recorded by:** the update track (`UPD`), 2026-10-11, for [351-UPD-0016](../issues-done/351-UPD-0016-images-with-slots.done).

### Problem

`files` reads `kernel.elf` and the bootloader to show that whole files come back through `vfs_server` (`files/src/main.rs`). On an image made with `make_usb_image.py --slots` the kernel is in `MIND/A/kernel.elf` (or `MIND/B/` after an update), not at the root, so `files` reports `OPEN kernel.elf FAILED` there. The QEMU checks do not run `files` on such an image yet.

### Plan (a proposal; the tools track decides)

- Read `kernel.elf` from the root when it is there, else from `MIND/A` or `MIND/B`: the slot that booted is in `BootInfo` (`boot_slot.slot`).
- Say which one was read.

### Acceptance criteria

On the slot image of `tests/usb_image_smoke.py`, `files` reads the slot's kernel whole.

## `poweroff` and `shutdown`, and a menu entry to turn the machine off (000-KRN-0080)

**Recorded by:** the kernel track (KRN), 2026-10-11, at the maintainer's request: "there is no shutdown, and it is needed while we have to turn the system off".

### Problem

`reboot --off` writes the caches, stops the services and turns the machine off. Since 000-KRN-0080 it does so on x86 too (ACPI S5), not only on aarch64. But:

- nobody looks for it under `reboot`;
- `wm`'s menu has no entry for it;
- `help reboot` and `shell/src/power.rs` still say "aarch64 only".

### Plan (a proposal; the tools track decides)

- **In the shell and `console`.** `poweroff` and `shutdown` as names for `reboot --off`, with `-f` as there; `help` names them.
- **In `wm`.** A "Turn off" entry next to the restart, asking for confirmation as the restart does.
- **The help texts and `docs/tools` (EN, RU).** Turning off works on x86 and aarch64. Where the firmware names no S5, it says why.

### Acceptance criteria

- **In the shell suite.** `poweroff -f` ends QEMU on x86 and aarch64 (the kernel track's check does it for `reboot -f --off`).
- **In `wm`.** The menu entry is there.

## `wm`'s moving background: drawn by rows, paced to a deadline (000-KRN-0081)

**Recorded by:** the kernel track (KRN), 2026-10-11, at the maintainer's request: "the moving background jerks when the bright screen is on, and redrawing is slow".

### Problem

The kernel's part is 000-KRN-0081:

- the framebuffer was mapped uncached;
- `compositor` scanned the whole stride.

`wm`'s own part, read from the code at fast-test 76bae52 (estimates for the MacBook Pro's 2880×1800, 5.2 Mpx, 20.7 MB a frame):

- **`paint()`** (`wm/src/main.rs:397-402`) copies every background cell to `wm`'s screen one pixel at a time, through `backdrop.pixel()` with a division by `unit` (`wm/src/background.rs:480-483`) and bounds checks per pixel. That is about 50–90 ms a frame.
- **`pattern()`** (`background.rs:166-237`) computes 324 000 blocks a frame, with divisions per block. Rings also takes a square root per centre. That is about 5–85 ms by pattern.
- **Pacing.** `sleep(if busy {5} else {30})` (`main.rs:646`) comes after the work. Since 000-KRN-0065 a sleep of 30 ms lasts 30–40 ms. The phase moves in whole steps (`p/8`, `p/12`, …), so a slow frame jumps.
- **Mouse moves.** Every one redraws everything and re-copies every pixel window (`main.rs:463, 635`).
- **Contrast.** At high contrast more pixels change each frame, so the compositor writes more.

All told, `wm` runs at about 5–11 frames a second, unevenly.

### Plan (a proposal; the tools track decides)

- **`paint()` by rows.** Runs of background cells are filled row by row with one loop for each unit size and no division: 5–10 ms instead of 50–90. A row helper in `libmind` (`Screen::span`, the core of `libmind`) can be asked of the kernel track.
- **Pacing to a deadline.** Sleep until the next frame's deadline less one tick (10 ms), not a fixed 30 ms after the work. A finer phase, with interpolated sine values, moves every term every frame.
- **A cheaper pattern.** Per-column and per-row terms computed once a frame, and a table or incremental distances instead of the square root.
- **Only what changed.** Cells whose colour did not change are not repainted. Mouse moves redraw the cursor's surroundings only.

### Acceptance criteria

On the MacBook Pro with 000-KRN-0081, the moving background runs evenly at full contrast. `wm` records the time a frame takes; the `wm` suite reads it in QEMU.
