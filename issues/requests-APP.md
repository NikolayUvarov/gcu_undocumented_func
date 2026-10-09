# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

Numbered on the tools branch (2026-10-09): clocks and the RTC (000-APP-0012), `log:` and `efivar` in the tools (211-APP-0013), full screen and the list of windows in `wm` (211-APP-0014), `update` (351-APP-0029), `wifi` (550-APP-0033), the audit's A07 and A08 (175-APP-0035, 175-APP-0036) and the marked window (211-APP-0037). The requests below wait.

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

## The shell's commands in `wm`'s `console` (211)

**Recorded by:** the kernel track (KRN), 2026-10-09, for main task [211](211-intel-pc-from-a-sata-ssd.md) at the maintainer's request, after a run on the MacBook Pro.

### Problem

Before `wm` starts, the shell's commands all work on its screen. In `wm` the user has `console`, which starts programs and has a few built-ins of its own, but the shell's commands do not work there. `reboot` is the example the maintainer gave; `ps`, `kill`, `logs`, `svc`-like lifecycle commands, `sync` and the network diagnostics are others. The maintainer asks to be able to use the shell, with its commands, from `wm` too.

The commands need the shell's authorities (process control, which `REBOOT` requires, the lifecycle client, the operator's network client). `console` holds none of them, and a second shell with all of them in every window would spread them.

### Plan (a proposal; the tools track decides)

- `console` sends a line it does not know to the shell, over an endpoint the shell lends it, and shows the answer. The shell runs the command on its own authority, as if typed on its screen, and sends back what it printed.
- The shell decides which commands it takes from `console`. Those that change the machine (`reboot`, `halt`, `kill`) ask for confirmation in the console window, as the consent prompts do.
- Or a window that is a view of the shell's own session. Either way only the shell holds the authorities.
- The help in `console` lists the shell's commands it accepts.

### Acceptance criteria

The `wm` suite opens `console`. `ps` there lists the tasks. `reboot` there, once confirmed, resets the machine (QEMU exits under `-no-reboot`). A command the shell does not take from `console` is refused with a message.

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

## A program that ends at once in a window leaves its message on view (158, 211)

**Recorded by:** the kernel track (KRN), 2026-10-09, after the maintainer's run on the MacBook Pro: "`camera` — nothing happens when it starts".

### Problem

Started from `wm`, `camera` found no camera and ended at once with `camera: no camera (the video gateway lists none)`. It printed to its text window (`WINDOW 1 OF PID 30: Text 256X112`), and the window closed when the program ended, so the user saw nothing. Every program that fails at its start behaves so in `wm`.

### Plan (a proposal; the tools track decides)

- When a program in a text window ends with a nonzero status, `wm` (or `libmind::windowed`) keeps the window, with its last lines and `ENDED (STATUS n): PRESS A KEY`, until a key or a click.
- A program that ends with status 0 closes its window as now.

### Acceptance criteria

The `wm` suite starts `camera` without a camera (or another program that fails at once): its window stays with the message until a key.

## `date set` in the shell (211-KRN-0051)

**Recorded by:** the kernel track (KRN), 2026-10-09, for [211-KRN-0051](211-KRN-0051-setting-the-clock.md), at the maintainer's request: a way to set the time.

### Problem

`date` prints the RTC's time. Nothing sets it, and the MacBook Pro's clock reads 2022-01-01. `rtc.wit` 1.2 adds `set` for a client with the setting badge, which init gives the shell (211-KRN-0051).

### Plan (a proposal; the tools track decides)

- `date set YYYY-MM-DD HH:MM[:SS]` calls `rtc::set` through the shell's setting client and prints the new time. It says the clock keeps no time zone.
- The help and `docs/tools` (EN, RU) describe it.

### Acceptance criteria

The `tools` suite sets a date and reads it back with `date`, on x86 and aarch64.

## Note: the shell's text for `loader.wit` 1.7 `unreadable` (211-KRN-0050)

The kernel track added `unreadable` to the loader's errors. The shell's match on loader errors is exhaustive, so the interface change took the shell's mapping with it: `ERR_IO` → `CANNOT READ THE PROGRAM: ITS DISK DOES NOT ANSWER (UNPLUGGED?)` in `shell/src/main.rs`. The tools track may word it otherwise. `wm`, `fm` and `console` print loader errors with `{:?}` and show `Unreadable`.

## Russian speech is barely intelligible on the MacBook Pro (252)

**Recorded by:** the kernel track (KRN), 2026-10-09, at the maintainer's request after a run on the MacBook Pro: "Russian audio output is barely understandable, very poor, with clicks". The task is to find out whether it can be fixed.

### Problem

`tts` runs its 16 kHz formant synthesizer and upsamples to 48 kHz for `audio_gw`. On the Mac's speakers Russian is hard to follow. The clicks are looked for in the driver (551-DRV-0010: polled playback without an interrupt). How intelligible the voice is, is the synthesizer's.

### Plan (a proposal; the tools track decides)

- Measure first. Run the Vosk check of 252 on the phrases the maintainer used, on the 16 kHz output and on the upsampled 48 kHz stream, to tell the synthesis from the upsampling.
- Then the cheapest gains: the upsampler's filter, and the Russian rules and voice parameters. 252's neural synthesis is the larger step.

### Acceptance criteria

A measured intelligibility before and after, and on the Mac the maintainer understands a Russian test sentence without clicks.

## Note: the camera is lent without a question (158; the maintainer's rule, 2026-10-09)

The maintainer ruled that a program the user starts gets the devices it is for without a question (CONTRIBUTING.md, "No question about a tool's own purpose"). At that instruction the kernel session removed the shell's `ASKS FOR THE CAMERA. ALLOW?` and changed `shell/src/main.rs`, `docs/tools` (EN, RU), `camera`'s help and the `video` suite's camera check. The tools track may revise the wording. Questions stay where an action goes beyond the tool's purpose: the firmware's boot settings, the network policy.

