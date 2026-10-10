# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

Numbered on the tools branch (2026-10-09): clocks and the RTC (000-APP-0012), `log:` and `efivar` in the tools (211-APP-0013), full screen and the list of windows in `wm` (211-APP-0014), `update` (351-APP-0029), `wifi` (550-APP-0033), the audit's A07 and A08 (175-APP-0035, 175-APP-0036), the marked window (211-APP-0037), the message a program that ends at once leaves in its window (211-APP-0039), the shell's commands in `wm` (211-APP-0040), Russian speech on the MacBook Pro with the kernel track's measured note on its hiss and clicks (252-APP-0041) and `date set` (211-APP-0042). Numbered on 2026-10-10: the camera from `wm` and the shell in a window, as the camera from `wm` and `console` (158-APP-0043), the shell's own window (211-APP-0040) and `console` joined to the shell (211-APP-0044, waiting for `SLOT_SHELL` from `KRN`). The requests below wait.

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

## `cpus` names AVX-512 and AMX (174-KRN-0037)

**Recorded by:** the kernel track (KRN), 2026-10-09, for [174-KRN-0037](174-KRN-0037-every-vector-state-component.md).

### Problem

The kernel now saves AVX-512's and AMX's state components where the processor has them. `STAT_CPUS.xsave` carries them (XCR0: `0xE0` AVX-512, `0x60000` AMX), and `BootInfo.cpu_features` has `FEATURE_AVX`, `FEATURE_AVX512` and `FEATURE_AMX`. `shell/src/observe.rs` prints `FPU=XSAVE+AVX` for any of them, so a user cannot tell from `cpus` that the wider units are usable.

### Plan

`cpus` prints `FPU=XSAVE+AVX`, then `+AVX512` and `+AMX` for each group whose bits are all set. The `XSAVE+AVX` prefix stays, because the `busy` and `smp` suites match it.

### Acceptance criteria

The `smp` suite with `--cpu-model max` still sees `FPU=XSAVE+AVX` on every CPU. A machine with AVX-512 shows `+AVX512` (the profile records it).

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

## The shell's window opens only when the user asks (211-APP-0040, 211-APP-0044)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's report from the MacBook Pro (`fast-test` a00618b): "a program started by itself when `wm` started (the shell). It should not start by itself, only at the user's request from `wm`'s menu."

### Problem

211-APP-0040 opens the shell's window whenever the shell starts a window manager (`shell/src/main.rs`: "when the shell starts a program that asks for the window manager client, it first opens a text window titled `shell`"). So it appears at every start of `wm`, unasked.

### Plan (a proposal; the tools track decides)

- `wm` starts with no shell window.
- The user opens it from `wm`'s menu (a `shell` item, under System or at the top), and by a key. Ctrl+Alt+F5 already opens it.
- The menu item reaches the shell through its command endpoint, which is 211-APP-0044's: `SLOT_SHELL` comes from the kernel track in 211-KRN-0058, now in its gate on the way to `main`.
- Until then, the item can be left out rather than the window opened at the start.

### Acceptance criteria

- The `wm` suite starts `wm` and finds no shell window.
- The menu item opens one, and closing it with Alt+W leaves `wm` running.
- On the MacBook Pro, `wm` starts with the desktop alone.

## A window back to its content's size (158, 211)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request after a run on the MacBook Pro: "the picture should stretch with the window, and the window should be able to take the size of its content, to get the original display back".

### Problem

- **The picture now follows the window.** `camera` scales its picture to whatever size `wm` gives its window (the kernel track's 158 change, f760714 on its branch).
- **But nothing brings a window back to the size of its content.**
  - For `camera` that is the stream's size, 320×240 by default, where the picture is drawn pixel for pixel.
  - `wm` updates `Win::size` when the program takes a new size (`wm/src/main.rs`, the resize handling), so `Win::natural()` follows the current size, not the first.

### Plan (a proposal; the tools track decides)

- **Keep the first size.** `wm` keeps the size a pixel window opened at (the content size its program asked for), beside its current one.
- **A "fit to content" command** sets the frame back to that size. It asks the program for it as a resize does, and keeps the frame on the screen.
  - For example a key (`Alt+0`), a double click on the title, or a title-bar button beside zoom.
  - The help screen and the top bar's key list name it.
- **Text windows:** the same command can take the frame to the text's own size, if that is meaningful there.

### Acceptance criteria

- The `wm` suite opens `camera` (the synthetic source), resizes its window and sees the picture scaled.
- The command brings the frame back to 320×240, and the picture is then the test pattern pixel for pixel.
- On the MacBook Pro the FaceTime camera's window returns to its first size.
