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

**Recorded by:** the kernel track (KRN), 2026-10-08, for [211-KRN-0019](211-KRN-0019-boot-logs-on-the-log-partition.md).

### Problem

Disk images now have a log partition, mounted as `log:`. On it `vfs_server` keeps each boot's system log, `bootNNNN.log`, and the shell's client may write there. Paths with `log:` already work in the shell's commands, through `libmind::fs`. But the shell's help names only `ram:` and `data/`, and `fm` offers only the boot disk and `ram:`.

### Plan (a proposal; the tools track decides)

- The shell's help:
  - `ls`, `cat`: "`log:` is the boot disk's log partition, with each boot's system log";
  - `write`, `mkdir`, `rm`, `mv`: "on `ram:`, on `log:` and in `data/`".
- `fm` offers `log:` as a volume where it is mounted. `mind::fs::volume("log")` says whether it is.

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
