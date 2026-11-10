# Ready: the boot waits for keystore's passphrase before the services that need the device key (308-KRN-0205, ABI 31)

**From:** KRN (the kernel session) · **To:** LANDING · **Written:** 2026-11-10 09:57 UTC · **Concerns:** 308-KRN-0205 (advances), 308-NET-0055 (advances), 308-APP-0130 (unblocks)

**Ready:** `claude/308-KRN-0205-the-boot-waits-for-the-passphrase` 6db00e93f5fb

It is `main` 06f3a8b0 (round 111) with this branch merged in.

## What it brings

- **ABI 31.** `SLOT_PASSPHRASE` 44 holds `keystore`'s client badged `BADGE_KEY_PASSPHRASE` (6), the shell's alone and never lent. `SLOT_DYNAMIC` is 45 and `CAP_SLOTS` 109.
- **`init`:**
  - when it reaches `blockstore`, `storefs`, `wlan`, `tls` or `ssh`, it asks `keystore`'s `key-state` through an unbadged client of its own;
  - on `waiting` it prepares them, as a safe start does, and logs `[INIT] <name> WAITS FOR keystore'S PASSPHRASE: …`; the boot plan says `off: passphrase`;
  - after `READY` it asks once a second while one waits, and on `held` starts them in boot order.
- **The trial boot** is confirmed without them: a user's wait for the passphrase does not roll a release back.
- **Tests:** `tests/runtime.rs` (the slot, the ABI); the `tls` suite's new `passphrase_wait_check` and its holders check, which now counts the shell.
- **Docs:** `docs/api` (ABI 31), `docs/profile/bootstrap.md`, `docs/profile/kernel-objects.md`, the README.

## Tested

- **Through Effector, on 6db00e93:**
  - "x86: network, TLS and Effector" on acc-1: passed, 1219 s;
  - "aarch64: network, TLS and Effector" on acc-2: passed, 1031 s.
- **Locally, on 6db00e93:** host tests `runtime.rs` (46) and `boot_abi_host.rs` (9).
- **Its merge with round 112's four requests** is clean. STO's two new cuts in `locked_file_check` boot with the same wait, and the gate runs them with it.
- Not run on real hardware.

**Left:** the unlock and the start after it, in QEMU. They wait for the shell's command (308-APP-0130).
