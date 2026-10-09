# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

Numbered on the tools branch (2026-10-09): clocks and the RTC (000-APP-0012), `log:` and `efivar` in the tools (211-APP-0013), full screen and the list of windows in `wm` (211-APP-0014), `update` (351-APP-0029), `wifi` (550-APP-0033), the audit's A07 and A08 (175-APP-0035, 175-APP-0036) the marked window (211-APP-0037), the message a program that ends at once leaves in its window (211-APP-0039) the shell's commands in `wm`'s `console` (211-APP-0040, waiting for a slot from `KRN`) and Russian speech on the MacBook Pro (252-APP-0041). The requests below wait.

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

## Note: the camera is lent without a question (158; the maintainer's rule, 2026-10-09)

The maintainer ruled that a program the user starts gets the devices it is for without a question (CONTRIBUTING.md, "No question about a tool's own purpose"). At that instruction the kernel session removed the shell's `ASKS FOR THE CAMERA. ALLOW?` and changed `shell/src/main.rs`, `docs/tools` (EN, RU), `camera`'s help and the `video` suite's camera check. The tools track may revise the wording. Questions stay where an action goes beyond the tool's purpose: the firmware's boot settings, the network policy.

## Note: the hiss and the clicks in Russian speech, measured (252-APP-0041)

**Recorded by:** the kernel track (KRN), 2026-10-09, for the tools track's 252-APP-0041.

The maintainer hears not only clicks but a periodic hiss in the synthesized sounds themselves.

**Measured by the kernel track** on the `tts` suite's recording (`/tmp/mind-core-tts.wav`, QEMU's wav backend at 44.1 kHz; the averaged spectrum of the loud frames):

| Band | Share of the energy |
|---|---|
| 0–4 kHz | −0.1 dB |
| 4–8 kHz | −16.7 dB |
| 8–12 kHz | −24.7 dB |
| 12–16 kHz | −46.3 dB |
| 16–24 kHz | about −40 dB |

- **The hiss is likely the upsampler's images.** A 16 kHz synthesizer has nothing above 8 kHz. Yet 8–12 kHz holds energy only 8 dB below the sibilants' 4–8 kHz.
  - `tts` upsamples 16 → 48 kHz by linear interpolation (`tts/src/main.rs`, `Upsampler`). Its response, sinc² of f / 16 kHz, leaves the image of each component at 16 kHz − f only 4 to 21 dB down.
  - So every fricative and burst gets a mirrored hiss at 8–12 kHz, which a laptop's small speakers make prominent.
- **The clicks are likely the onsets.** After the synthesizer's pauses (exact silences of 23, 65, 123 and 285 ms), several onsets rise from 0 to 3000–5000 within two or three samples, with no ramp. Examples: 450.9 ms, 2698.0 ms and 4130.0 ms of the recording.
  - The driver's path showed no underrun in QEMU: no silences of a buffer's length inside the speech.
  - The polled playback on the Mac is still looked at in 551-DRV-0010.

What the kernel track would try (the tools track decides):

- The upsampler: a polyphase low-pass FIR instead of linear interpolation. For example, 48 taps (16 a phase) of a windowed sinc with its cutoff near 7 kHz at 48 kHz, which puts the images 50 dB or more down. Measure the 8–12 kHz band again: it should fall well below −40 dB.
- The onsets: a ramp of a few milliseconds where a segment starts or ends at silence, and bursts limited in their slope.
