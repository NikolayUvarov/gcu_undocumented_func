# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

Numbered on the tools branch (2026-10-09): clocks and the RTC (000-APP-0012), `log:` and `efivar` in the tools (211-APP-0013), full screen and the list of windows in `wm` (211-APP-0014), `update` (351-APP-0029), `wifi` (550-APP-0033), the audit's A07 and A08 (175-APP-0035, 175-APP-0036), the marked window (211-APP-0037), the message a program that ends at once leaves in its window (211-APP-0039), the shell's commands in `wm`'s `console` (211-APP-0040, waiting for a slot from `KRN`), Russian speech on the MacBook Pro with the kernel track's measured note on its hiss and clicks (252-APP-0041) and `date set` (211-APP-0042). The requests below wait.

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

## The camera from `wm`, and the shell in a window (158, 211)

**Recorded by:** the kernel track (KRN), 2026-10-10, for main tasks [158](158-video-capture.md) and [211](211-intel-pc-from-a-sata-ssd.md), at the maintainer's request after a run on the MacBook Pro (`fast-test` bc681376b8d8). The maintainer chose that the tools track does it.

### Problem

- **The camera works from the shell's screen.** `camera` there streamed the FaceTime HD camera: 64 frames at 320×240, none broken (`log:boot0001.log`, `video_gw` and `usb_host` lines at 27.7–34.1 s).
- **From `wm` it does not.** Started from the menu or from `wm`'s `console`, it ends with `camera: no camera was granted (start camera from the shell and allow it)`:
  - `wm` does not ask the shell for the camera (`mind::request!` in `wm/src/main.rs` has no `REQUEST_CAMERA`), so it holds nothing in `SLOT_CAMERA`;
  - `wm`'s `start()` and `console`'s `run()` do not handle `REQUEST_CAMERA`, so a program they start never gets it.
- **The maintainer asks for more:**
  - `camera` in `wm` shows its stream in its window, started from the menu and from `console`;
  - `console` from the right-click menu works fully, as the shell does;
  - the shell itself runs in a window in `wm`;
  - the difference between `console` and the shell, and its reason, is explained to the user.

### Plan (a proposal; the tools track decides)

- **The camera.**
  - `wm` asks for `REQUEST_CAMERA`. The shell lends it without a question, by the maintainer's rule "No question about a tool's own purpose".
  - `wm` lends `SLOT_CAMERA` to a program that asks for it, and so does `console` to what it starts. `console` asks for it too.
- **The shell in a window.**
  - With [211-APP-0040](211-APP-0040-the-shells-commands-in-console.md), `console` joined to the shell is the shell in a window. The menu can offer it as `shell`.
  - A program that needs what `wm` does not hold (the network, the camera, the log, the lifecycle client) could be started by the shell on its own authority, through `shell.wit`. It then opens its window in `wm`, rather than being started by `console` with `wm`'s fewer grants.
  - Either way only one shell holds the operator's authorities.
- **The kernel track's part:** `SLOT_SHELL` (and `SLOT_CLIPBOARD`), asked for in `requests-KRN.md` for 211-APP-0040 and 000-APP-0032. The kernel track takes it now as a task of its own.
- **The explanation:** a section in `docs/tools` (EN, RU) and in `console`'s help:
  - the shell is the one holder of the operator's authorities;
  - `console` is a terminal window that asks the shell for them;
  - why a second full shell in every window is not made: it would spread the authority to reboot, kill, change the network policy and the firmware's boot order.

### Acceptance criteria

- The `wm` suite starts `camera` from the menu and from `console` with the video gateway's synthetic source, and sees its window show the stream.
- The `shell` item opens a window where `ps` lists the tasks, `reboot` asks and, once confirmed, resets the machine, and `camera` shows the stream.
- On the MacBook Pro, `camera` shows the FaceTime camera in a `wm` window.
