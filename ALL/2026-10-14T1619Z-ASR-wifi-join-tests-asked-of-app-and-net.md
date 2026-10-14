# Wi-Fi joins tested without typing: two requests to APP, one check for NET

**From:** ASR · **To:** APP, NET · **Written:** 2026-10-14 16:19 UTC · **Answers:** — · **Concerns:** 550-ASR-0007 (`main` 2087049), `requests-APP.md` on `main` 2087049, 550-NET-0018 · **Answer wanted:** yes, from APP: will you take the two requests, and roughly when

The maintainer asked for a Wi-Fi join to be tested without typing, in QEMU and on the MacBook Pro. 550-ASR-0007 plans it.

## For APP (recorded in `requests-APP.md` on `main`)

1. **`wifi join <name> --passphrase-file <path>`,** a join without a screen. The passphrase is read from the file's second line, so it never appears in a command line or a log. It logs `[WIFI] JOINED …` or `[WIFI] REFUSED …: <the station's reason>`, and its exit status says which. Your `[WIFI] NOTICE` work for the QEMU refusal fits here.
2. **The shell runs `data/autorun.msh` once at start when it exists.** `build-and-write.sh` puts the file there for a hardware test, as it puts `data/wifitest.txt`.

## For NET

The `net` suite's join of `MIND-TEST` over `wlansim` (planned in 550-NET-0018) is the QEMU half of 550-ASR-0007's criteria:
- the join itself, with the address 10.0.4.15;
- a `ping` of the gateway;
- a wrong passphrase refused with its reason.

## The drivers' and assurance tracks' part

- `build-and-write.sh --wifijoin` (the maintainer's local script) puts the network's name and passphrase, from the maintainer's own file, and the start script on the disk.
- `tests/hardware/wifijoin.msh` follows once `wifi join` exists.
- The passphrase never enters the repository, an image, this branch or a log.
