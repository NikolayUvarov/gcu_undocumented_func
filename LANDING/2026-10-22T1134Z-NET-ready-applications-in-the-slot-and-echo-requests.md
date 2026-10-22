# Ready: a release's applications staged in its slot (351-UPD-0023), and the echo requests (000-NET-0039, 000-NET-0040)

**From:** NET and UPD (the storage session) · **To:** LANDING · **Written:** 2026-10-22 11:34 UTC · **Concerns:** 351-UPD-0023, 000-NET-0039, 000-NET-0040

**Ready:** `claude/relaxed-meitner-5bmhpz` f2cb4105b3c81a6ab6fe8c4a5412bc32801c2ddd

## What it brings

- **351-UPD-0023, the updater's part (advanced, not closed).**
  - `updater` stages every application a release's manifest lists into the slot, with the boot files. An application the volume holds as listed, in the running slot or at the root, is copied whole, not fetched.
  - `apply` and `rollback` refuse a slot that holds some applications but not all, or one the manifest does not list. A slot with none passes, as before.
  - `verify_other` reads the kept channel before hashing the slot. On the slow i7 under TCG, hashing took longer than a session parser's 30 s idle limit, the channel read failed, and the trial never started.
  - `plan::kind` splits a manifest into boot files, applications and shared files, with a host test.
  - The `update` suite:
    - release 6 changes `uptime.elf`; only its blob is fetched, and slot B holds all applications as listed;
    - with 351-KRN-0151, slot A runs the root's applications, and the trial runs slot B's;
    - it prints the updater's log if the trial is not confirmed.
  - Docs: `docs/update/updater.md` 0.3 and `slots.md`, EN and RU.
- **000-NET-0039 (closed):** an ICMP socket per echo request. An echo to a neighbour that never answers no longer holds the card's queue. At most 16 echo requests wait at once.
- **000-NET-0040 (closed):** `idl/socket.wit` 2.6 `ping-with`, an echo of 0 to 1472 bytes of a chosen pattern, its reply's size and data checked; `netcheck echo`; the fuzzer covers it.
- **Text:** `requests-NET.md` without the requests these took.

## `main` in it

c045eaa8, text only on top of round 9's f8d76dd9.

## What was run

- **This session's container** (QEMU 8.2, TCG, 4 CPUs):
  - on 0faf8518 (f2cb4105 adds text only): x86 `update`, `linkupdate` (with 351-KRN-0151's slot check) and `net`; aarch64 `update`;
  - on d49f63f7 (`main` c9300679 with ABI 15): host tests `updater`, `netpolicy`, `idl_fuzz`; x86 `net`, `update`, `linkupdate`.
- **Effector:**
  - "aarch64: files, network and TLS" passed on the i7 (`DESKTOP-i7-930-wsl-u24`) for d49f63f7 merged with `main` c9300679, with the `verify_other` fix. Before that fix, it had failed there in `update`.
  - On un-rizen it passed for 4caf6024 (000-NET-0039 and 0040) merged with `main` f7630021.
  - Not run on a maintainer's machine since round 9 merged: 351-KRN-0151's change and my loader checks in the `update` suite ran in this container only.
