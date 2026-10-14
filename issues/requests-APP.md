# Requests for the tools track (APP), not numbered yet

**Owner:** tools track · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-07

The tools track numbers its own tasks (`NNN-APP-MMMM`), so requests from other tracks wait here. The tools track turns each into a task and removes it from this file, and the file goes when it is empty.

Numbered on the tools branch (2026-10-09): clocks and the RTC (000-APP-0012), `log:` and `efivar` in the tools (211-APP-0013), full screen and the list of windows in `wm` (211-APP-0014), `update` (351-APP-0029), `wifi` (550-APP-0033), the audit's A07 and A08 (175-APP-0035, 175-APP-0036), the marked window (211-APP-0037), the message a program that ends at once leaves in its window (211-APP-0039), the shell's commands in `wm` (211-APP-0040), Russian speech on the MacBook Pro with the kernel track's measured note on its hiss and clicks (252-APP-0041) and `date set` (211-APP-0042). Numbered on 2026-10-10: the camera from `wm` and the shell in a window, as the camera from `wm` and `console` (158-APP-0043), the shell's own window (211-APP-0040) and `console` joined to the shell (211-APP-0044, waiting for `SLOT_SHELL` from `KRN`); the shell's window only when asked (211-APP-0045) and a window back to its content's size (211-APP-0046); later that day the menu's tests category (176-APP-0051), voice control as a service (000-APP-0052), what `hear` heard (000-APP-0053), the help keys in the shell's commands (000-APP-0054), the date and time from `wm`'s Settings (000-APP-0055), every menu entry reacting (000-APP-0056) and the machine's memory in `top` and `free` (211-APP-0057); then `cpus` naming AVX-512 and AMX (174-APP-0058); on 2026-10-12 the battery in `wm` and the shell (211-APP-0070); on 2026-10-14 the two races in the `wm` suite, a title written after adoption and Turn off's Enter (000-APP-0073). The requests below wait.

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
- **Pacing to a deadline.** Sleep until the next frame's deadline, not a fixed 30 ms after the work: since 000-KRN-0083 a sleep ends within a millisecond of the time asked. A finer phase, with interpolated sine values, moves every term every frame.
- **A cheaper pattern.** Per-column and per-row terms computed once a frame, and a table or incremental distances instead of the square root.
- **Only what changed.** Cells whose colour did not change are not repainted. Mouse moves redraw the cursor's surroundings only.

### Acceptance criteria

On the MacBook Pro with 000-KRN-0081, the moving background runs evenly at full contrast. `wm` records the time a frame takes; the `wm` suite reads it in QEMU.

## `files` reads the kernel from the slot that booted (351-UPD-0016)

**Recorded by:** the update track (`UPD`), 2026-10-11, for [351-UPD-0016](../issues-done/351-UPD-0016-images-with-slots.done).

### Problem

`files` reads `kernel.elf` and the bootloader to show that whole files come back through `vfs_server` (`files/src/main.rs`). On an image made with `make_usb_image.py --slots` the kernel is in `MIND/A/kernel.elf` (or `MIND/B/` after an update), not at the root, so `files` reports `OPEN kernel.elf FAILED` there. The QEMU checks do not run `files` on such an image yet.

### Plan (a proposal; the tools track decides)

- Read `kernel.elf` from the root when it is there, else from `MIND/A` or `MIND/B`: the slot that booted is in `BootInfo` (`boot_slot.slot`).
- Say which one was read.

### Acceptance criteria

On the slot image of `tests/usb_image_smoke.py`, `files` reads the slot's kernel whole.

## The shell's `effector` command: setup, policy, status (750)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [750](750-effector-agent.md) at the maintainer's request.

### Problem

The Effector agent is off by default, and only the owner may turn it on and say what the server may do. Only the shell can write to `data/`, so the owner's commands for the agent belong in the shell.

### Plan (a proposal; the tools track decides)

- `effector setup URL PIN [NEXT-PIN]`:
  - writes the server's address and certificate pins to `data/effector/`;
  - reads the token from the console without echoing it;
  - asks for confirmation and shows what it wrote, except the token.
- `effector policy show | edit | allow <line> | deny <line>`: the policy of 750-EFF-0003, with confirmation for anything that widens it.
- `effector enable | disable`: through `svc enable`/`disable` (173) for both services; disabling also revokes the session.
- `effector status`:
  - connected or not;
  - the server and its pin;
  - the last heartbeat accepted;
  - the operations in flight;
  - the last audit lines.
- The same commands in `msh` under `requires: lifecycle`, and help in `docs/tools/`.

### Acceptance criteria

In QEMU, the shell sets up the agent, enables it, and the 750-EFF-0006 suite sees it online. `policy deny` takes effect on the next request; `disable` stops both services.

## Note: the shell shows what a script prints at once (176-KRN-0087; the maintainer's word, 2026-10-12)

**Recorded by:** the kernel track (KRN), 2026-10-12.

The maintainer's full run (`diag/full.msh`) looked hung for 42 s, then printed everything at once. The shell drew its screen only between commands, and a script is one command. At the maintainer's word ("all this must be fixed"), the kernel session changed two lines in `shell/src/msh.rs`:

- the script host's `print` draws the screen after each line;
- so does a command a script runs without capturing it.

The `shell` suite now checks the screen while a script sleeps. The tools track may do it otherwise.

## Screenshots and recordings for the project's page (000-KRN-0093; the maintainer's request, 2026-10-12)

**Recorded by:** the kernel track (KRN), 2026-10-12, for [000-KRN-0093](000-KRN-0093-pictures-and-video-for-the-showcase.md).

### Problem

The maintainer will shoot the interface on the MacBook Pro for the page on GitHub: pictures, a row of frames and a video. `screenshot` and `record` do the capture, but:

- **The shell is in the picture.** A screenshot is typed in the shell, and `wm`'s menu entry opens over the screen.
- **The files are lost at a reboot.** Without a file name both write to `ram:`.
- **A Retina recording is heavy.** At 2880 × 1800 each Motion JPEG frame is large, and the encoder may not keep 10 frames a second.

### Plan (a proposal; the tools track decides)

- **A key for a screenshot** that saves at once, without the shell or a menu on the screen: PrintScreen on a PC keyboard. The MacBook Pro has no PrintScreen key, so a combination as well, for example Ctrl+Alt+S. A short note in the corner says where the file went.
- **`screenshot -d <seconds>`:** waits first, so the screen can be arranged.
- **Files kept by default** in `data/media/` (`screen-NNN.bmp`, `record-NNN.avi`), so they survive a reboot and can be read from the stick in Windows; `ram:` when the boot disk is read-only.
- **`record -s 2`:** half the size in each direction. **`record -q <quality>`:** the JPEG quality. Both for a Retina screen.

### Acceptance criteria

- **In the `tools` suite**, the key writes a BMP to `data/media/` with the screen as it was, no shell in it.
- **`record -s 2 -t 3`** writes an AVI whose frames are half the screen's size.
- **On the MacBook Pro**, the maintainer takes a picture and a 30 s recording without the shell in them.

## Redirection and pipes in the shell and in `msh` (the maintainer's request, 2026-10-12)

**Recorded by:** the kernel track (KRN), 2026-10-12. The kernel's part, a text input for programs, is [000-KRN-0094](000-KRN-0094-a-text-input-for-programs.md).

### Problem

The maintainer: "redirection and pipes have to be added to the shell". `dmesg -s battery > file.txt` fails, as does `dmesg | grep BATTERY`.

### Plan (a proposal; the tools track decides)

- **`cmd > file` and `cmd >> file`:**
  - a command of the shell's own writes what it would print;
  - a program's output is read from its console queue (`SYSCALL_CONSOLE_READ`), as the shell reads it for the screen, and written to the file;
  - the file is written through the shell's file client: `ram:`, `data/`, `log:`.
  - This needs nothing from the kernel and can come first.
- **`cmd < file` and `a | b`**, on [000-KRN-0094](000-KRN-0094-a-text-input-for-programs.md):
  - the shell starts `b` with a text input: `loader.wit` 1.9 `commit-with-input` (or with `front`, as `commit-in-front`) returns the input's writer, a `mind::io::Writer`;
  - it copies `a`'s output (or the file) into it with `write_all`, which waits while the kernel's 4 KiB queue is full, and closes it with `close` when `a` ends (dropping the writer ends the input too);
  - `b` reads it with `mind::io::read` or `Lines`, to 0 at its end; a program started without one gets `Error::NotFound` (`has_input`);
  - `b`'s output goes to the screen, a file or the next `|`;
  - Esc stops the whole line;
  - the line's result is the last command's.
- **Tools read their input when no file is named:** `grep`, `sha256`, `cat`, `find`, `view`.
- **`msh`:** the same `>`, `>>`, `<` and `|` in a command's words, and a command's output as a value (`let text = output(dmesg -s battery)`).
- **The help screen, `docs/msh.md` and `docs/tools`** (EN, RU).

### Acceptance criteria

- **In the `tools` suite:**
  - `ls data > ram:l.txt`, then `cat ram:l.txt` shows the listing, and `>>` adds to it;
  - `dmesg | grep SHELL` prints only matching lines;
  - `cat log:boot0001.log | sha256` equals `sha256 log:boot0001.log`;
  - 100 KiB through `|` arrive whole.
- **On the MacBook Pro**, `dmesg -s battery > data/battery.txt` leaves the file on the stick.

## Display output over several monitors (177; the maintainer's request, 2026-10-12)

**Recorded by:** the kernel track (KRN), 2026-10-12, for main task [177](177-display-output.md).

The maintainer asked for several monitors, their positions, and the GPU for the picture. 177 plans it in steps; this track's part is step 2's `wm` side: each output's rectangle, windows placed per output, a `displays` tool and `wm`'s Settings writing `data/displays.txt`. The kernel track does steps 0–1 (every output and its EDID, every screen the firmware lit, in `BootInfo`) first, and the interface versions the later steps need. Nothing here waits yet: number it when you start.


## `wifi` follows `idl/wifi.wit` 1.0 as agreed (550-APP-0033)

**Recorded by:** the network track (`NET`), 2026-10-12, answering the tools track's proposal in requests-NET.md ("`idl/wifi.wit`: what `wifi` needs of the station").

### Problem

The network track agreed the interface in [`idl/wifi.wit`](../idl/wifi.wit) 1.0 (550-NET-0018), with these changes to the proposal:

- **An SSID is `list<u8, 32>`, not `string<32>`.** An access point sends up to 32 bytes, and some send names that are not UTF-8 (GBK, Latin-1). As bytes, a network is listed and joined by its exact name; `wifi` shows it as it can (for example, with U+FFFD for bytes that are not UTF-8).
- **`status` has no `address`, and `reason` no `no-address`.** The station does no DHCP: the address is the network stack's (`ip`, which the network page of `wm`'s Settings already opens). `status` gains the BSSID, the channel and the data rate.
- **New cases:**
  - `security.other`: TKIP, management frame protection required, or another AKM. Like WEP, WPA3 and enterprise, the station does not join it.
  - `error.not-found` and `reason.not-found`: no network of that name in the last scan.
  - `reason.unsupported`.
  - `state.scanning`.
- **`scan` answers when the scan ends** (about 2 s), with the list. While joining or joined, it answers the list as it stands and does not leave the channel.
- **The badges.** The station's one endpoint also serves `idl/net.wit` to the network stack. A client with `BADGE_WIFI` (`mind::network`) speaks `wifi.wit` and only reads. With `BADGE_WIFI_CHANGE`, it may also join, leave and forget.

### Plan (a proposal; the tools track decides)

`wifi`'s `Station` and `Demo` follow 1.0 in a step of 550-APP-0033: the stand-in serves the agreed interface, and the passphrase rules stay as the program has them (the station checks them again).

### Acceptance criteria

The tools suite checks `wifi` against a stand-in station of `idl/wifi.wit` 1.0.

## The shell lends a TLS client that cannot present the device certificate (351-NET-0022)

**Recorded by:** the network track (`NET`), 2026-10-12, answering the tools track's request "A TLS client without the device certificate".

### Problem

`tls` refuses `client-certificate` to a client badged `BADGE_TLS_NO_DEVICE` (`mind::network`, `idl/tls.wit` 1.2). The shell lends its own unbadged client today, so a program can still sign in as the device.

### Plan (a proposal; the tools track decides)

Once `init` gives the shell the right to badge its `SLOT_TLS` copy (requests-KRN.md, "The right to badge the shell's TLS client"), the shell does two things:
- for `REQUEST_TLS`, it lends a copy minted with `BADGE_TLS_NO_DEVICE`;
- it keeps the unbadged one for its own `https -c`.

### Acceptance criteria

In the `tls` suite, a program with the lent client gets `denied` for `client-certificate`, and the shell's `https -c` still sends the certificate.

## `ip` shows each card's kind and the default card (550-NET-0025)

**Recorded by:** the network track (`NET`), 2026-10-12, for the request "Several network interfaces" in requests-NET.md ("`ip` shows the default and the kind of each card").

### Problem

`netstack` now states which card is the default: the card the policy's `default card N` line prefers while its link is up and configured, else the first configured card, wired before the Wi-Fi station. `idl/socket.wit` 2.5 `cards` tells each card's kind (wired or wireless), whether it is there, up and configured, and which one is preferred and which is the default. The shell's `ip` shows none of this yet.

### Plan (a proposal; the tools track decides)

- `ip` adds to each card's line its kind and the words `DEFAULT` and `PREFERRED` where they apply, from `cards`, and lists the station's card even when it is not there yet.
- A live choice that does not outlast a restart, `ip prefer N|none`, through the operator's `prefer`, if the tools track wants one. The lasting choice stays the policy line, which `netpolicy add` asks the user for.

### Acceptance criteria

In the `net` suite's two-card check, `ip` shows card 0 as `DEFAULT`, and card 1 as `PREFERRED DEFAULT` after `netpolicy add default card 1`.

## The shell logs its first frame (213-KRN-0098)

**Recorded by:** the kernel track (KRN), 2026-10-13, for [213-KRN-0098](213-KRN-0098-black-screen-when-the-shell-takes-it.md): on the maintainer's Ryzen machine the screen went black when the shell took it, and the system log cannot tell a shell that never drew from a screen that never showed it.

**Asked:** one line in the system log when the shell has rendered its first frame, for example `[SHELL] FIRST FRAME 2560X1440 AT 7.9 S`, and one if a call made before it takes over 5 s, naming the service.

**Acceptance criteria:** the line is in the boot log of every suite's machine and of a real one.

## A note: a launch session now holds every launch slot (000-KRN-0101)

**Recorded by:** the kernel track (KRN), 2026-10-14, for [000-KRN-0101](000-KRN-0101-a-launch-session-holds-every-launch-slot.md), the tools track's request for 211-APP-0044.

**What changed:** `loader`'s launch session keeps a grant for every launch slot and `SLOT_FIRMWARE` (18), not 5. `check`'s `launch: every slot` lends all 17 launch slots in one session.

**Asked:** nothing is required. `wm` and the shell may lend `SLOT_SHELL` and the battery without putting them last, and the `wm` suite can check `console` holding the shell's commands beside `record -w`'s window lease.

## A note: `check`, `bench` and `kbench` stop when their console is gone (176-KRN-0103)

**Recorded by:** the kernel track (KRN), 2026-10-14, for [176-KRN-0103](176-KRN-0103-the-tools-stop-when-their-console-is-gone.md), the tools track's request for 000-APP-0056.

**What changed:** libmind's `mind::output::gone()` says that the launcher lent an endpoint for the output and it is gone. The three tools look at it after each line they show: they write their log, saying they stopped because their console closed, and end with exit status 3. `check`'s `a tool's console closes` drops kbench's console after four pieces of output and sees it end.

**Asked:** the `wm` suite's walk may close their windows without waiting: `RUN_TO_END` can be empty. Other programs keep running when their console goes, as before; one that should stop can look at `gone()` too.

## A note: the safe start after a run that stopped early (000-KRN-0076)

**Recorded by:** the kernel track (KRN), 2026-10-14, for [000-KRN-0076](000-KRN-0076-a-frozen-machine-restarts-and-starts-safely.md).

**What changed:** a run that did not end as a task asked (a freeze the chipset's watchdog reset, a cut power) leaves a mark in a firmware variable, and the next boot is a safe start: `init` logs `[INIT] SAFE START: THE LAST RUN DID NOT END AS ASKED (A FREEZE, A RESET OR A CUT POWER); NOT STARTED: bcm_wifi, video_gw, audio_gw, tts` and leaves those stopped; `svc start` starts each.

**Asked:** the shell (and `wm`) show that line to the user at the start, with how to start what was left out. Nothing waits on it.

## `app`, `app2` and `dzen-clock`: libmind's drawing, no `static mut`, the right words on exit (the critique of 2026-10-14)

**Recorded by:** the kernel track (KRN), 2026-10-14, from the critique [issues-critica-done/02](../issues-critica-done/02_code_and_comliance_analysis.md), §3, checked against `main` dfc42ee. TRACKS.md names no track for the demo programs. They are user programs, so they come here. If the tools track does not take them, say so here and the kernel track will.

### Problem

- **`app/src/main.rs`:**
  - `MAIN_SP`, `THREAD_SP`, `WORK_COUNTER` and `THREAD_STACK` are `static mut` (:12–18). Every access goes through `addr_of_mut!` with volatile reads and writes, so nothing goes wrong today. It is still the pattern Rust is retiring.
  - `background_task` (:36) never returns but is not `-> !`. Were it to return, it would jump to address 0. The critique's claim that the first thread's stack is misaligned is wrong: :47–48 leave the slot of a return address, and the stack is 16-aligned as SysV asks.
  - It draws with its own `draw_char`, `draw_string` and `usize_to_str` (:63–80), with no bounds check, straight into `fb_ptr`. libmind has `mind::gfx::Screen` and `mind::util::Decimal` for this.
- **`app2/src/main.rs`:**
  - It has its own `fill_rect` and `draw_text` (:14–40).
  - On Esc it logs `[APP2] ESC PRESSED. RETURNING TO KERNEL.` (:88), but it returns to the shell.
  - It reads the sprite through `as_mut_slice()` (:118) where `as_slice()` does.
- **`dzen-clock/src/main.rs:79, 183`** has the same "returning to kernel" wording. tests/qemu_smoke.py:2829 checks it, so the test changes with it.

### Plan (a proposal; the tools track decides)

- `app`: atomics for the three counters and pointers, and an `UnsafeCell` wrapper for the stack; `fn background_task() -> !`; `Screen` and `Decimal` for its drawing. Keep the pixel colours the `normal` suite checks (qemu_smoke.py:7461–7470: cyan, and red after Space).
- `app2`: `Screen` for its rectangles and text; "RETURNING TO THE SHELL"; `as_slice()`.
- `dzen-clock`: the same words, with its check in qemu_smoke.py.

### Acceptance criteria

- The `normal`, `busy`, `smp` and `isolation` suites pass on x86 and aarch64. They use `app` and `app2` as workloads, and `boot` runs `app`.
- `grep -rn "static mut" app app2` and `grep -rn "RETURNING TO KERNEL"` find nothing.

## The shell lends a client of the SSH service (351-NET-0027)

**Recorded by:** the network track (`NET`), 2026-10-14, for [351-NET-0027](351-NET-0027-the-ssh-service.md).

### Problem

The service `ssh` (351-NET-0027) reads files over SFTP for programs, logged in as the device with a key that only `keystore` holds. A program reaches it as it reaches `tls`: through a client the shell lends.

### Plan (a proposal; the tools track decides)

Once `init` gives the shell its client in `SLOT_SSH` and a request flag `REQUEST_SSH` exists (requests-KRN.md, "The SSH service in the boot services"), the shell lends a copy of its client in the program's same slot for `REQUEST_SSH`, together with the program's flow grant, as it does for `REQUEST_TLS`. `download` (the network track's) then reads `sftp://` addresses through it.

### Acceptance criteria

In the `net` suite, `download sftp://…` started from the shell reads a file from the host's OpenSSH server through the lent client.
