# Request: `wifi join` without a screen, and the shell's start script `data/autorun.msh`

**From:** ASR · **To:** APP · **Written:** 2026-10-14 16:21 UTC · **Answers:** — · **Concerns:** 550-ASR-0007 (`main` 5ca263c), 550-APP-0033; it replaces the pointer to `requests-APP.md` in `ALL/…-ASR-wifi-join-tests-asked-of-app-and-net.md` (requests go here now, the maintainer's rule; `main` 5ca263c took the copy out of `requests-APP.md`) · **Answer wanted:** yes: will you take it, and roughly when

## Why

The maintainer asked for Wi-Fi joins to be tested on the MacBook Pro without typing (550-ASR-0007). Two things stand in the way:
- **`wifi` is interactive only.** A join needs the passphrase typed into its screen.
- **Nothing starts at boot on the maintainer's request.** The `diag/` scripts run when typed in the shell.

## Asked (a proposal; the tools track decides)

- **`wifi join <name> --passphrase-file <path>`.** It joins without a screen and reads the passphrase from the file's second line (the first line is the name), so the passphrase never appears in a command line or a log.
  - It logs `[WIFI] JOINED <name>` with the channel, or `[WIFI] REFUSED <name>: <the station's reason>`.
  - Its exit status says which.
  - `wifi status` prints the link's state the same way.
- **The shell runs `data/autorun.msh` once at start when it exists,** in its own window, as if typed. With no such file nothing changes. `build-and-write.sh` puts it there for a hardware test, as it puts `data/wifitest.txt`.

## Acceptance

- **In QEMU, a join without typing.** With `wlansim`, a script holding `wifi join MIND-TEST --passphrase-file data/wifijoin.txt` joins, and `ip` then shows 10.0.4.15.
- **A wrong passphrase** logs the refusal's reason.
- **The start script.** `data/autorun.msh` runs once at the shell's start, and only when present.
