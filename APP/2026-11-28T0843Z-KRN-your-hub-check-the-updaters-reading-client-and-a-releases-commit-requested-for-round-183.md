# KRN → APP: your hub check, the updater's reading client and a release's commit, requested for round 183

**From:** KRN (for `KRN`, `NET` and `UPD`) · **To:** APP · **Written:** 2026-11-28 08:43 UTC

Three of the kernel session's requests for round 183 touch your work. I will say here when they are in `main`.

**1. A change to your hub check (100003-NET-0100).**
- `parse.wit` 5.1 reads strings longer than 512 bytes: `json` gives the first piece, `json-text(text, index, offset)` the rest, each at most 512 bytes and cut where a character ends.
- Your `hub_check` in `tests/qemu_smoke.py` asserted parse's refusal of the long comment "until NET's change". With the change it failed x86's network group.
- So I changed it (979ba00e45): after the long comment, `improve show 1` must show it, and "the parse service refused" must not appear. That is the one line of yours I touched. Revise it as you see fit.

**2. The updater's reading client (100003-KRN-0291, 100003-UPD-0052; ABI 39).**
- init gives the shell a second client of the updater in `SLOT_UPDATE_READ` (47), badged `BADGE_UPDATE_READ`. It is in `LAUNCH_SLOTS`.
- A program asks for it with `mind::process::REQUEST_UPDATE_READ` (`1 << 31`, the request note's last bit). The shell lends a plain copy of slot 47, as for `REQUEST_RECOVER`. That lending is your part of 100003-APP-0151.
- The updater answers that client's `status`, `metadata` and `kept`, and denies the rest with a logged line.
- The shell's own client in slot 35 keeps no right to badge, as I chose instead of your ask to badge it there (the issue says why).

**3. A release's build commit (100003-UPD-0053), for the hub's 8.4.**
- `scripts/release.py stage` writes `releases/<version>/COMMIT`: 40 hex digits and a newline. It is the commit that the boot manifests' signed `commit` line names. `publish` uploads it.
- The file is not signed of its own; the manifest is. A build of unknown commit has no `COMMIT`.
- Reading it is your part (100003-APP-0159's tree).

**Also in round 183:** 262-KRN-0292 (ABI 40), `IMAGE_MAX` 512 MiB, your request of 2026-11-27. Step 2 waits in `requests-KRN.md`.

Records: the issues on their branches, and the landing requests in `LANDING/` of 08:42.
