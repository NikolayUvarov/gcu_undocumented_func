# 550-APP-0078 — `wifi join` and `wifi status` without a screen

**Type:** tools (`wifi`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open: in `main` since 92781301 (2026-10-20), checked in QEMU; `ip`'s address after the join and the MacBook Pro's run left · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-11.4, MC-11.5, MC-12.1

**Recorded by:** the tools track (APP), 2026-10-14, from the assurance track's request on `internal-dev-messages` (`APP/2026-10-14T1621Z-ASR-wifi-join-without-a-screen-and-a-start-script.md`), for [550-ASR-0007](550-ASR-0007-wifi-join-tests.md): the maintainer asked for Wi-Fi joins to be tested without typing.

## Problem

`wifi` is interactive only: a join needs the passphrase typed into its screen. A test on the MacBook Pro, or a script, cannot join a network.

## Plan

- **`wifi join <name> --passphrase-file <path>`:**
  - joins without a screen, through the station's client the shell lends (550-APP-0033);
  - reads the passphrase from the file's second line; its first line must be the name, or the join is refused. So the passphrase is in no command line, script or log;
  - asks the station to remember it with `--remember`, which `keystore` keeps;
  - waits for the station's answer, 60 s at most.
- **What it reports:**
  - it logs `[WIFI] JOINED <name> <BSSID> CHANNEL <n> <rate> MB/S`, `[WIFI] REFUSED <name>: <the station's reason>` or `[WIFI] NO ANSWER <name>`;
  - its exit status says which: 0 joined, 1 refused, 2 no answer, 3 no station or a bad argument;
  - it prints the same line for the shell's console and for a script's `run` result.
- **`wifi join <name>`** without a file is for an open network, or one the station remembers.
- **`wifi status`** prints the link's state as one line (`[WIFI] STATUS JOINED …`, `OFF`, `FAILED <reason>`), and `wifi leave` leaves.
- **The help text** and `docs/tools` (EN, RU).
- **The passphrase file** is the maintainer's, on their own disk (550-ASR-0007). `wifi` never writes, logs or echoes its contents. A script may remove the file after a join it remembered.

## Acceptance criteria

- **The `net` suite** (x86, with `wlansim`):
  - `wifi join MIND-TEST --passphrase-file data/wifijoin.txt` exits 0 and logs `JOINED MIND-TEST 02:00:00:57:4C:A0`;
  - `ip` then shows the station's address;
  - a file with a wrong passphrase exits 1 with the station's reason;
  - a file whose first line names another network is refused;
  - the passphrase is nowhere in the serial log.
- **`wifi status` and `wifi leave`** after the join.
- **The MacBook Pro** (550-ASR-0007): the script's join of the maintainer's network is in the boot log.

## Progress (2026-10-14)

- **Where:** `wifi/src/headless.rs` and `wifi/src/main.rs`, on the task branch `claude/550-APP-0033-wifi-station-client` (822f44e, 1887276) and in `fast-test`.
  - `wifi join`, `status` and `leave` open no screen.
  - The passphrase file's buffer and the passphrase are cleared after the join.
  - A station that keeps no networks is asked again without `--remember`, as on the screen.
- **Host tests:** `tests/wifi_host.rs`, 12 in all.
  - The command line and the passphrase file.
  - A join against the stand-in: joined, a wrong passphrase, an unknown network, no station, a station that does not remember.
  - A join that never ends gives `NO ANSWER` after 60 s.
- **The `net` suite** (x86, `wlansim`), passed on PCU on 1887276:
  - a file naming another network is refused;
  - a wrong passphrase is refused with the station's reason (`the passphrase is wrong`);
  - `MIND-TEST` is joined (`JOINED MIND-TEST 02:00:00:57:4C:A0 CHANNEL 6 …`);
  - `wifi status` and `wifi leave` work, and `wifi join` alone prints the usage;
  - no passphrase is on the serial line.
- **This is QEMU's stand-in radio, not the MacBook Pro's** (MC-12.1).
- **In `main`** since 92781301 (2026-10-20): the gate on 92781301 passed (every x86 group on i9, every aarch64 group on un-rizen-service, the rest locally).
- **The check of `wifi join`'s usage failed twice in others' gates** (ASR's local gate on aarch64; NET's gate 22 on x86, i9 quiet): the prompt came back before the usage line. `wifi` prints its line through the system log and ends at once, so on the serial line the shell's prompt can come first. The `net` suite now takes each of wifi's lines from the serial line or, when it is not there yet, from `dmesg -s wifi`, where it must come once more than before the command (2026-10-20).
- **`ip`'s address after the join** is checked now: the network track's half of 550-ASR-0007 is in `main`, and the `net` suite reads `IP 10.0.4.15/24` and the station's card (550-APP-0085).
- **Left:** the MacBook Pro's run.

## Related

[550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`), [550-ASR-0007](550-ASR-0007-wifi-join-tests.md) (the tests), [000-APP-0079](000-APP-0079-the-shells-start-script.md) (the start script), 550-NET-0018 (`wlan`, `wlansim`).
