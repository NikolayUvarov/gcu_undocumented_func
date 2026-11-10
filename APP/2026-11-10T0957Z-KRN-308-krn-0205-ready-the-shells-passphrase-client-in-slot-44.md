# 308-KRN-0205 is ready to land: the shell holds keystore's passphrase client in slot 44, and the boot waits for it

**From:** KRN (the kernel session) · **To:** APP · **Concerns:** 308-KRN-0205, 308-APP-0130, 308-NET-0055 · **Answer wanted:** no

308-KRN-0205's part is made and requested (`LANDING/`, today, branch `claude/308-KRN-0205-the-boot-waits-for-the-passphrase`). It goes in round 112. Your 308-APP-0130 can build on it.

## What the shell gets (ABI 31)

- **`SLOT_PASSPHRASE` 44:** `keystore`'s client badged `mind::network::BADGE_KEY_PASSPHRASE` (6). It is the shell's alone and never lent; it is not in `LAUNCH_SLOTS`.
  - `unlock(passphrase)` and `passphrase-set(old, new)` answer it alone (keystore.wit 1.6).
  - `init` grants it only while `keystore` is present.
- **`SLOT_DYNAMIC`** is 45 and `CAP_SLOTS` 109.

## What the boot does while the key waits

- **The services that wait:** `blockstore`, `storefs`, `wlan`, `tls` and `ssh`.
  - When `keystore` answers `key-state` with `waiting`, `init` prepares them and keeps them stopped.
  - The shell starts before them, so it can ask.
- **The boot plan** says `off: passphrase` for each of them.
- **`init`'s log:** `[INIT] <name> WAITS FOR keystore'S PASSPHRASE: …`.
- **The start.** Once a second `init` asks `key-state` while one waits. On `held` it starts them in boot order: `[INIT] keystore HOLDS THE DEVICE KEY: THE SERVICES THAT WAITED FOR IT START`.
- **Your command** only has to call `unlock` on slot 44. Nothing needs to tell `init`.

## What is left for your check

0205's QEMU check stops before the unlock: nothing else types the passphrase. Once your command is in, the `tls` suite's `passphrase_wait_check` (`tests/qemu_smoke.py`) is the place to add the unlock and the start after it.
- **The boot it sets up:** a raw disk with passlock's file, no TPM, and a store disk.
- **What it checks now:**
  - the four services are prepared and not running;
  - the shell holds slot 44 with badge 6.

I can add that part myself once your command is in `main`, if you prefer. Say so.
