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

## The shell lends its TLS client for `REQUEST_TLS`

**Recorded by:** the network track (NET), 2026-10-08, for [351-NET-0002](351-NET-0002-https-for-programs.md).

### Problem

Once the kernel track adds `REQUEST_TLS` (`issues/requests-KRN.md`, "A TLS client for programs"), a program asking for it should get the shell's client of the TLS service, as it gets the window broker or the pin service. `download` needs it for `https://` URLs.

### Plan (a proposal)

- In `Shell::start_with`, lend `SLOT_TLS` to the program's `SLOT_TLS` when it asks for `REQUEST_TLS`, the shell holds the client, and the program also gets a flow grant (TLS without a flow is of no use to it). A script must declare `tls` (issue 094).
- The `wm` line of missing grants names `tls` as it names `network`.

### Acceptance criteria

`download data/x.bin https://…` runs a TLS session over `download`'s own grant; a script that does not declare `tls` runs it without one.
## The shell's `effector` command: setup, policy, status (700)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [700](700-effector-agent.md) at the maintainer's request.

### Problem

The Effector agent is off by default, and only the owner may turn it on and say what the server may do. Only the shell can write to `data/`, so the owner's commands for the agent belong in the shell.

### Plan (a proposal; the tools track decides)

- `effector setup URL PIN [NEXT-PIN]`:
  - writes the server's address and certificate pins to `data/effector/`;
  - reads the token from the console without echoing it;
  - asks for confirmation and shows what it wrote, except the token.
- `effector policy show | edit | allow <line> | deny <line>`: the policy of 700-EFF-0003, with confirmation for anything that widens it.
- `effector enable | disable`: through `svc enable`/`disable` (173) for both services; disabling also revokes the session.
- `effector status`:
  - connected or not;
  - the server and its pin;
  - the last heartbeat accepted;
  - the operations in flight;
  - the last audit lines.
- The same commands in `msh` under `requires: lifecycle`, and help in `docs/tools/`.

### Acceptance criteria

In QEMU, the shell sets up the agent, enables it, and the 700-EFF-0006 suite sees it online. `policy deny` takes effect on the next request; `disable` stops both services.
