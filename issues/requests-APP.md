# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

## Clocks ask the RTC service 10 times a second

### Problem

`clock` calls `mind::rtc::seconds_since_midnight()` after every `wait_or_exit(100)`: 10 IPC calls a second per clock.

On x86 each answer is about 14 CMOS port accesses, each a system call. With 60 clocks that came to 8 700 port system calls a second. On aarch64 with 120 clocks, the RTC service's queue was full all the time. A `date` from the shell waited 8.5 s for a place (`000-KRN-0010` covers the kernel's side).

`sysmon` reads `STAT_TASKS` and `STAT_ENDPOINTS` every 100 ms. 171-KRN-0009 made both one pass, but they still cost about a millisecond each under the lock with 170 tasks.

### Plan (a proposal; the tools track decides)

- A clock could read the RTC once and count seconds from `CLOCK` (monotonic nanoseconds), reading the RTC again every minute or so.
- `sysmon` could sample endpoints less often than tasks.

### Acceptance criteria

The RTC service's load does not grow by 10 calls a second with each clock.

## `update` in the shell and `msh` (351, phase 2)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [351](351-self-update.md) at the maintainer's request.

### Problem

The `updater` service ([351-UPD-0007](351-UPD-0007-updater-service.md)) will have an interface (`idl/update.wit`) but no command.

### Plan (a proposal; the tools track decides)

- `update check | fetch | apply | status | rollback` in the shell, and the same in `msh` under `requires: lifecycle`.
- `apply` asks for confirmation and names the versions.
- `status` shows the running, staged, trial and last-known-good versions and the last error.
- `sysmon` or `top` may show a staged update.

### Acceptance criteria

The commands drive the updater against the test server in QEMU, and the tools suite checks them.

## A Wi-Fi setup program (550)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [550](550-network-on-real-hardware.md) at the maintainer's request.

### Problem

With the 802.11 station that the network track is asked for ([requests-NET.md](requests-NET.md)), the system could join a Wi-Fi network, but nothing lets a person choose one and type its passphrase.

### Plan (a proposal; the tools track decides)

- `wifi` (proposed name): a program in a `wm` window or on a full screen, and the same as shell commands. It:
  - lists the networks found with their name (SSID), signal, security (open, WPA2, WPA3, enterprise) and channel, and refreshes the list;
  - connects with a passphrase typed without echo;
  - offers to remember the passphrase, and stores it through `keystore`, never in a file;
  - shows the state (connecting; connected, with the address; failed, with the reason) and forgets a remembered network.
- It talks only to the station's Wi-Fi configuration interface. It never sees frames, or keys once the passphrase is handed over.

### Acceptance criteria

The tools suite checks the program against a station stand-in with fixed scan results (QEMU has no Wi-Fi). On the MacBook Pro, it lists the networks around and joins a WPA2-PSK one.

## Full screen for a window in `wm` (211)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [211](211-intel-pc-from-a-sata-ssd.md) at the maintainer's request, after the first run on a MacBook Pro.

### Problem

`Alt+Enter` maximizes a window within the desktop: its frame and the top bar stay. The maintainer asks for a key that gives a window the whole screen, without the frame or the bar, and a way back to where it was.

### Plan (a proposal; the tools track decides)

- A key, for example `Alt+F` or `F11`, toggles full screen for the window in front.
  - In full screen the window's content covers the whole screen, and the top bar is hidden.
  - The same key, or `Esc` held down, restores the frame the window had before. That may be a maximized or snapped frame.
- A pixel window gets the screen's size, as a maximized one does (window broker, issue 163). A text window gets the whole cell grid.
- The help screen and the top bar's key list name the key.

### Acceptance criteria

The `wm` suite checks that:

- the key gives a text window and a pixel window the whole screen, with no frame or bar;
- the same key restores the earlier frame;
- `Alt+Tab` from a full-screen window still works.

## A list of the windows in `wm` (211)

**Recorded by:** the kernel track (KRN), 2026-10-08, for main task [211](211-intel-pc-from-a-sata-ssd.md) at the maintainer's request.

### Problem

`Alt+Tab` steps through the windows one by one. Nothing lists them all, so a hidden or covered window is found only by stepping.

### Plan (a proposal; the tools track decides)

- A key, for example `Alt+L`, and an item of the top bar open a list of the windows.
  - Each entry has its title, its program's PID, and its state: in front, hidden, maximized or full screen.
- Arrows and `Enter`, or a click, bring the chosen window to the front. `Alt+W` closes the selected one from the list.
- The list follows windows that open and close while it is shown.

### Acceptance criteria

The `wm` suite opens three windows and lists them. It brings the second to the front from the list, by key and by click, and checks that a closed window leaves the list.

## The log volume `log:` in the shell's help and in `fm` (211)

**Recorded by:** the kernel track (KRN), 2026-10-08, for [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md). **Reported again by the maintainer on 2026-10-09, P1:** on the MacBook Pro `fm` shows only `A:` and `ram:`, not the log partition of the same disk.

**Nothing waits on other tracks.**
- `vfs_server` mounts the partition as `log:`.
- `mind::fs::volume("log")` (and `"models"`) says whether a volume is mounted.
- `wm` and the shell lend `fm` the shell's VFS client (`REQUEST_FILES`), which reads and writes `log:`.

What is missing is in `fm` alone: `VOLUMES` in `fm/src/fm.rs` (line 122) is fixed to `A:` and `ram:`. Its messages about where one may write (lines 27, 153 and 429) name only `ram:` and `data/`.

### Problem

Disk images now have a log partition, mounted as `log:`. On it `vfs_server` keeps each boot's system log, `bootNNNN.log`, and the shell's client may write there. Paths with `log:` already work in the shell's commands, through `libmind::fs`. But the shell's help names only `ram:` and `data/`, and `fm` offers only the boot disk and `ram:`.

### Plan (a proposal; the tools track decides)

- The shell's help:
  - `ls`, `cat`: "`log:` is the boot disk's log partition, with each boot's system log";
  - `write`, `mkdir`, `rm`, `mv`: "on `ram:`, on `log:` and in `data/`".
- `fm` offers `log:` as a volume where it is mounted, in the Alt+F1/F2 menu and the panels' volume line. `mind::fs::volume("log")` says whether it is. `models:` (the model disk, read-only) the same way.
- `fm`'s messages say `log:` is writable, as `ram:` and `data/` are.

### Acceptance criteria

The `tools` suite finds `log:` in `help` where the image has the partition. `fm` lists `log:` and shows a boot log in its viewer, in the USB image test or a suite booted from an image with the partition.

## `efivar` in the tools guide and `help` (351)

**Recorded by:** the kernel track (KRN), 2026-10-08, for [351-KRN-0027](../issues-done/351-KRN-0027-uefi-variables.done).

### Problem

`efivar` is a new console program. It lists the firmware's boot variables (`BootCurrent`, `BootNext`, `BootOrder`, `Boot####`) and sets `BootNext` and `BootOrder`. The shell lends it the firmware privilege only after the user agrees, and a script must declare `firmware`. The kernel track wrote it to test its system call. The tools guide (`docs/tools`, EN and RU) and the shell's `help` do not name it. Scripts cannot declare `firmware` (it is not in `msh`'s words), so a program a script starts runs without it.

### Plan (a proposal; the tools track decides)

- A row in `docs/tools/README.md` and `README_RU.md`: what `efivar` shows, its write forms (`bootnext`, `bootorder`, `delete bootnext`, `append db|dbx|KEK <file>`), the consent prompt, on x86 and aarch64.
- Whether a script may declare `firmware` (the user would still be asked each time) is the tools track's decision.
- `df` and `fsck` list `log:` as they list `models:` (211-KRN-0019).

### Acceptance criteria

The guide and `help efivar` describe it. The `tools` suite runs `efivar --help`.

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

## Saving does not destroy an existing `<name>.tmp` (audit A07, main task 175)

**Recorded by:** the kernel track (KRN), 2026-10-09, routing the 2026-10-09 audit ([175](175-audit-2026-10-09.md)) at the maintainer's decision. **APP may start it now.**

### Problem

Audit finding A07 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)), confirmed, P2.
- `edit` saves through `File::create` (`MODE_TRUNCATE`).
- `fm`'s editor calls `create(&temporary, true)`, mapped to `MODE_TRUNCATE` (`fm/src/main.rs`, 82–84).
- So saving `x` destroys an unrelated `x.tmp`, and two editors saving one file collide. `MODE_NEW` exists (`libmind/src/fs.rs`, line 18).

### Plan and acceptance (from the audit)

- The staging file is created exclusively (`MODE_NEW`), with another name on a collision, and the save tracks the one it owns.
- **Tests:** an existing staging name, concurrent saves, and cleanup after a failure, without changing another file.
- `issues-audit/repro/fm_repro.py`'s A07 part becomes the regression test.

## `fm` keeps a move's source until the destination is on its medium (audit A08, main task 175)

**Recorded by:** the kernel track (KRN), 2026-10-09, routing the 2026-10-09 audit ([175](175-audit-2026-10-09.md)) at the maintainer's decision. **APP may start it now; P1.**

### Problem

Audit finding A08 ([audit](../issues-audit/2026-10-09-repository-audit.md), [assessment](../issues-audit/2026-10-09-repository-assessment.md)), confirmed.
- **The order.** Removals are planned after the copies (`fm/src/fm.rs`, 559–562), and `finish()` flushes the sources before the target (599).
- **The lost error.** `Disk::flush` returns `()` (line 63), and `fm/src/main.rs:94` drops the error of `root.flush()`.
- **The result.** In a move between two durable volumes (`data/` to a USB stick), the source's removal reaches its medium before the destination's data. A later I/O error or power loss loses the file, and `fm` reports it moved.

### Plan and acceptance (from the audit)

- Flushing returns its error. A source is removed only after its destination flushed successfully. When persistence fails the source stays and `fm` shows the failure.
- **Tests:**
  - destination write-back and flush errors;
  - multi-file moves, retry and cancellation;
  - save and copy completion propagate flush errors.
- `issues-audit/repro/fm_repro.py`'s A08 part becomes the regression test.

## The window being dragged is marked (211-DRV-0018)

**Recorded by:** the kernel track (KRN), 2026-10-09, for [211-DRV-0018](211-DRV-0018-macbook-trackpad-gestures.md), at the maintainer's request after a run on the MacBook Pro.

### Problem

The trackpad now drags a window by its title: a press held while a finger moves, and, as the kernel session is adding now, a double tap that keeps the button down until the next tap. While a window moves, nothing shows that `wm` holds it, so the user cannot tell whether the drag took.

### Plan (a proposal; the tools track decides)

- While `wm` drags or resizes a window by the pointer, the window is marked. For example, its title bar in the focus colour inverted, or its frame drawn double, until the button is released.
- The mark is quiet, and the same for a mouse and a trackpad.

### Acceptance criteria

The `wm` suite starts a drag by a title, sees the mark while the button is held and its absence after the release.

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
