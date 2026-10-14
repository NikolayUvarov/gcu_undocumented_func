# 550-APP-0078 — `wifi join` and `wifi status` without a screen

**Type:** tools (`wifi`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open (planned) · **Blocked by:** — (it builds on 550-APP-0033's task branch, which waits for the kernel track's ABI 10 in `main`) · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track G · **Constitution:** MC-11.4, MC-11.5, MC-12.1

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

## Related

[550-APP-0033](550-APP-0033-wifi-setup-program.md) (`wifi`), [550-ASR-0007](550-ASR-0007-wifi-join-tests.md) (the tests), [000-APP-0079](000-APP-0079-the-shells-start-script.md) (the start script), 550-NET-0018 (`wlan`, `wlansim`).
