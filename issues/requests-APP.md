# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

Numbered on the tools branch (2026-10-09): clocks and the RTC (000-APP-0012), `log:` and `efivar` in the tools (211-APP-0013), full screen and the list of windows in `wm` (211-APP-0014), `update` (351-APP-0029), `wifi` (550-APP-0033), the audit's A07 and A08 (175-APP-0035, 175-APP-0036), the marked window (211-APP-0037), the message a program that ends at once leaves in its window (211-APP-0039), the shell's commands in `wm` (211-APP-0040), Russian speech on the MacBook Pro with the kernel track's measured note on its hiss and clicks (252-APP-0041) and `date set` (211-APP-0042). Numbered on 2026-10-10: the camera from `wm` and the shell in a window, as the camera from `wm` and `console` (158-APP-0043), the shell's own window (211-APP-0040) and `console` joined to the shell (211-APP-0044, waiting for `SLOT_SHELL` from `KRN`); the shell's window only when asked (211-APP-0045) and a window back to its content's size (211-APP-0046). The requests below wait.

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

## `top` and `free` show the machine's memory, not only the kernel's arena (211)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's question after a run on the MacBook Pro: "why is the available memory shown as 64 MB, when the computer has gigabytes?"

### Problem

- **The memory bar shows the kernel arena.** `top`'s memory bar and line (`monitor/src/top.rs`, `m.used` of `m.arena`) show the kernel arena: the 64 MiB of kernel structures (tasks, endpoints, capability tables). So it reads "2 MB of 64 MB" as if that were all the memory.
- **The machine's memory is not shown, though the kernel sees it.**
  - On the MacBook Pro the firmware's map has 7.6 GiB of conventional memory, 5.7 GiB of it above 4 GiB.
  - The kernel's frame pool is `7768 MiB, 7679 MiB free` (`hw0001.txt`, "The kernel's choices").
  - `StatMemory` carries it already: `frames` and `frames_free`, in bytes. That pool holds the programs' images, stacks, screens, heaps and objects, so about 89 MiB were in use, not 2 MB.
- **`free` prints it on a second line** (`FRAMES=… FRAMES_FREE=…`), after `ARENA=…`, so it reads the same way.

### Plan (a proposal; the tools track decides)

- **`top`:** the first memory bar is the machine's memory: `frames - frames_free` of `frames` (named "memory" or "RAM"). The arena gets a second, smaller line named "kernel arena".
- **`free`:** the frame pool first, as "memory", then the arena as "kernel arena". `load`'s "kernel arena" series keeps its name.
- **`sysmon`'s and `wm`'s summaries, if they show memory:** the same.
- **`docs/tools` (EN, RU):** what the frame pool and the arena are.

### Acceptance criteria

- On QEMU with 512 MiB, `top` and `free` show about 400 MiB of memory with what is in use, and the arena separately.
- On the MacBook Pro they show about 7.6 GiB.

## A "Tests and performance" category in `wm`'s menu (176)

**Recorded by:** the kernel track (KRN), 2026-10-10, for main task [176](176-test-and-performance-utilities.md) at the maintainer's request.

### Problem

The kernel track is making three console programs in a new crate, `bench/`:

- `check`, a self-test of what is done;
- `bench`, the components' performance;
- `kbench`, the kernel's performance.

The maintainer wants them at hand from the menu as well as from the command line. With no category in `wm/src/menu.rs` (`CATEGORIES`), they land under "Other", next to `netbench` and `memtest`, which belong with them.

### Plan (a proposal; the tools track decides)

- A category "Tests and performance" with `check`, `bench`, `kbench`, `netbench` and `memtest`, each started as `console <name>`, so its table shows in a window.
- Their lines in `help` and `docs/tools` (EN, RU) once the programs are in `main` (the kernel track writes their own pages).

### Acceptance criteria

The five programs are under that category in `wm`'s menu and run in a `console` window.

## Voice control as a service one can turn on (the maintainer's request, 2026-10-10)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request.

### Problem

The maintainer asked for "a service that can be switched on for audio commands, so that the system can be run by voice".

Today voice control lives in one shell:

- `voice on` starts the `voice` program for that shell's console only, and it stops with it;
- F12 is push-to-talk, and only while that console has the keyboard;
- nothing turns it on at boot, and `wm` and its windows have no part in it.

### Plan (a proposal; the tools track decides)

- **A boot service**, off by default, that `svc enable voice` (173) turns on for the next boots and `svc start voice` for this one. It holds the microphone only while it listens.
- **How it listens.** It listens on push-to-talk from whichever program has the keyboard (the shell's F12, a key in `wm`), or continuously on a wake phrase if the maintainer chooses that later.
- **What it does.** It hands what it recognized to the shell's command endpoint (`SLOT_SHELL`, 211-KRN-0058), so the same confirmations hold: it asks before stopping a service or rebooting. In `wm` it can also open programs from the menu by name.
- **What it shows.** A mark that voice control is on and when it is listening, on the shell's line and in `wm`'s bar; every phrase heard and what was done with it goes to the system log.
- **Least authority (Art. 11.11).** It holds the audio, tts and read-only file clients and the shell's endpoint, nothing else.

### Acceptance criteria

- `svc enable voice` and a reboot leave voice control on, without a shell command.
- In QEMU, a WAV file standing in for the microphone opens a program and asks before a reboot.
- On the MacBook Pro, the same by the microphone once capture works there (551-DRV-0010).

## `hear` shows what it heard, for the operator (the maintainer's request, 2026-10-10)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request.

### Problem

The maintainer: "`hear` prints only errors now; it should print what it heard, so the operator can check it."

What it prints today:

- `HEARD "<phrase>" …` for an accepted command;
- `NOT UNDERSTOOD (CLOSEST "…", CONFIDENCE=…)` for a refused one;
- `NOT UNDERSTOOD (TOO SHORT)`, `NOTHING HEARD`;
- `HEAR: NO MICROPHONE …`, `HEAR: NO INPUT FROM THE MICROPHONE`, or `HEAR: THE MICROPHONE IS BUSY …`.

When the microphone gives silence or noise, every one of these reads as an error. Nothing tells the operator whether sound came in at all, how loud it was, or what the recognizer made of it.

### Plan (a proposal; the tools track decides)

- **While it listens:** a level meter on one line (the peak and RMS in dBFS, refreshed several times a second), and the threshold that starts an utterance.
- **For each utterance:** its length, its level, and the best three phrases with their confidence, then the verdict: accepted, refused below the threshold, or too short.
- **When nothing was heard:** the noise floor's level over the wait, so that "the microphone gives nothing" and "it was too quiet" are told apart.
- **`--save FILE`** keeps the utterance (or the whole wait) as a WAV file on `ram:` or `log:`, for the operator to send for analysis or to feed back with `hear --wav`. (`listen` already shows a level and plays a recording back, but separately from recognition.)
- **`voice`** prints the same per-utterance lines on its console.

### Acceptance criteria

- In QEMU, `hear --wav` of the test recordings prints the level, the length and three candidates for each utterance. With no input it prints the noise floor.
- On the MacBook Pro, the operator can see whether the microphone gives sound (551-DRV-0010 logs the capture's peak on the driver's side).

## The shell's commands answer the same help keys as programs (000-KRN-0066)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request: "`date` prints the date but answers none of `/help`, `--help`, `-help`; all programs must answer the help keys in the same way."

### Problem

- **Programs.** Since 000-KRN-0066, every program built with `libmind` answers `--help`, `-help`, `-h`, `/help`, `/h`, `/?` and `-?` (any case) with its text (`mind::process::HELP_KEYS`, `asks_help`).
- **The shell's commands.** `date`, `time`, `ps`, `ls` and the rest answer none of them. `date --help` gets "THIS COMMAND TAKES NO ARGUMENTS", and `ls --help` looks for a file. The same holds for the commands of `wm`'s `console`.
- **Programs with a screen.** The shell's `help_instead` (`shell/src/main.rs`) shows a screen program's text from its file only for `--help`. With `fm -h`, the program now prints its text and exits, out of sight.

### Plan (a proposal; the tools track decides)

- **In the shell and in `console`.** Before a command parses its arguments, `asks_help(args)` turns `<command> <help key>` into `help <command>`: the command's lines from `HELP`.
- **`help_instead`.** It uses `asks_help` in place of `args != b"--help"`.
- **`docs/tools` (EN, RU).** One line on the help keys.

### Acceptance criteria

- In the shell and in `console`, `date -h`, `date /?`, `ls --help` and `ps -help` print their lines from `help`.
- `fm /?` shows fm's text without starting it.
- The shell suite checks a few.

## The date and the time set from `wm`'s Settings (the maintainer's report, 2026-10-10)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request.

### Problem

The maintainer tried to set the time from `wm` and "it does not react to Enter."

The Settings page "Date and time" (000-APP-0048, `wm/src/settings.rs`) only shows text: "date set YYYY-MM-DD HH:MM[:SS] in the shell's window (Ctrl+Alt+F5) or on its screen". Nothing on it can be chosen or changed, so Enter does nothing. The maintainer's clock is wrong, and `wm` gives no way to correct it.

### Plan (a proposal; the tools track decides)

- **The page.**
  - It shows the RTC's date and time, refreshed each second.
  - It has fields for the year, month, day, hours, minutes and seconds. ↑/↓ or typed digits change a field; Tab moves between fields. "Now from the network" can come later, with NTP.
- **Applying it.** Enter (or a "Set" button) asks for confirmation ("Set the clock to 2026-10-10 14:05:00? It keeps no time zone"). It then sets the clock through the shell's command endpoint (`SLOT_SHELL`, `shell.wit`), as `date set …`. The shell alone holds the RTC client with the setting badge (`rtc.wit` 1.2, 211-APP-0042), and `wm` gets no such right of its own.
- **The result** shows on the page and in `wm`'s bar clock: the new time, or why it was refused (an invalid date, the shell's refusal).

### Acceptance criteria

- In the `wm` suite, the page sets the clock, and `date` in the shell then shows the new date.
- An invalid date (2026-02-30) is refused with its reason.
- On the MacBook Pro, the maintainer sets the clock from `wm`.

## Every entry of `wm`'s menu and Settings reacts (the maintainer's request, 2026-10-10)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's request: "all commands in `wm`'s menu must react adequately."

### Problem

An entry that does nothing on Enter or a click leaves the user guessing whether the system hung. The date page above is one case, and others may be like it.

### Plan (a proposal; the tools track decides)

- **An audit.** Go through every menu entry, every Settings page and row, and the right-click menu. Each must do one of these on Enter and on a click:
  - open its program (in a window, or in `console` for a console program);
  - change its setting visibly;
  - say why it cannot (the device is missing, the program is not on the disk).
- **The `wm` suite** walks them all. For each entry it checks that something visible happened: a window, a line in `console`, a changed row, or the reason.
- **Text-only pages** say so in their title, or become settable as above.

### Acceptance criteria

- The `wm` suite opens each menu entry and each Settings row and finds its reaction.
- No entry is silent.
