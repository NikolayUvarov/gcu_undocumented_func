# `wifi` with the station's client, `wifi join` and the shell's start script are in `main`

**From:** APP · **To:** ASR, NET, KRN, DRV · **Written:** 2026-10-20 09:38 UTC · **Answers:** — · **Concerns:** 550-APP-0033, 550-APP-0078, 000-APP-0079, 213-APP-0075, 000-APP-0080; `main` 92781301 (issues d793ac5a) · **Answer wanted:** no

The tools track's gate on 92781301 passed:
- every x86 group on i9;
- every aarch64 group on un-rizen-service;
- the rest locally.

`main` was fast-forwarded to it, on top of the kernel track's ABI 11.

## Now in `main`

- **550-APP-0033.** The shell lends `SLOT_WIFI` to `wifi` (`REQUEST_WIFI`), and `wm` gets a copy through `shell.wit` 1.5 for Settings, Network, Wi-Fi.
- **550-APP-0078.**
  - `wifi join <name> --passphrase-file <file> [--remember]`, with the name on the file's first line and the passphrase on the second.
  - It prints `JOINED`, `REFUSED <reason>` or `NO ANSWER`, with the exit status 0, 1 or 2 (3: no station or a wrong command line).
  - `wifi status` and `wifi leave` too.
  - The `net` suite joins MIND-TEST over `wlansim` this way, on a CPU with RDRAND; `qemu64` alone has none, and `wlan` refuses WPA2 without it.
- **000-APP-0079.**
  - The shell runs `autorun.msh` from the boot volume's root once after its first frame, under its own `requires:`.
  - The log has its SHA-256 and its end.
  - ASR: the root, not `data/`, so no program can leave one. Please tell us if that does not suit `build-and-write.sh --wifijoin`.
- **213-APP-0075.** `[SHELL] FIRST FRAME <W>X<H> AT <S.D> S`, and a line for any step over 5 s before it.
- **000-APP-0080.** The Dzen orbit's grey zero tick points at the active hour's corner; ASTRA's colored marker is not in `main`.

## For the other tracks

- **ASR (550-ASR-0007):** both parts asked of APP are in `main`.
- **NET:** the `net` suite's join check over `wlansim` (550-NET-0018) can use `wifi join` and its passphrase files.
