# 308-KRN-0205 — The boot waits for `keystore`'s passphrase before the services that need the device key

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** open (the kernel session's part in `main` since round 113, 2026-11-10, ABI 31; the unlock's check waits for the shell's command, APP's 308-APP-0130) · **Blocked by:** — (keystore's part of [308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md) is in `main`, round 80; its open criteria are this task's grant and wait; the kernel session's queue) · **Main task:** [308](308-encryption-at-rest.md), for [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md) · **Constitution:** MC-6.8, MC-9.6

## Problem

**Where a TPM seals the device key,** `keystore` unseals it at boot.

**Where none does,** the key will be sealed under the user's passphrase (308-NET-0055). Then `blockstore`, `tls`, `ssh` and `wlan` cannot start before the user types it.

## Plan

1. **`init`** starts the services that need the device key only once `keystore` says it holds it. It waits for that word from `keystore`, which waits for the passphrase.
2. **The console says why the boot waits.** The other services, and the shell, start as now.
3. **A safe start** asks for the passphrase as a normal one does.

## Acceptance criteria

- In QEMU without a TPM:
  - the store and the network's keyed services start only after the passphrase;
  - a wrong one leaves them unstarted, with the reason logged.
- With a TPM, the boot is as before.

## Progress (2026-11-10)

On the branch `claude/308-KRN-0205-the-boot-waits-for-the-passphrase`, ABI 31:

- **The shell's client of `keystore`.** `SLOT_PASSPHRASE` 44 holds `keystore`'s client badged `BADGE_KEY_PASSPHRASE`, the shell's alone and never lent.
  - Nothing held one before, so nobody could call `unlock` or `passphrase-set`.
  - `SLOT_DYNAMIC` is 45, and `CAP_SLOTS` 109.
- **`init`.** When it reaches a service that needs the device key, it asks `keystore`'s `key-state` through a client of its own, unbadged; any client may ask.
  - **The services:** `blockstore`, `storefs` (the store's file face), `wlan`, `tls` and `ssh` (`KEYED`).
  - **On `waiting`,** it prepares them, as a safe start does (000-KRN-0131): their capabilities are minted while it holds the platform privilege. It logs `[INIT] <name> WAITS FOR keystore'S PASSPHRASE: …`, and the boot plan says `off: passphrase`.
  - **After `READY`,** it asks every second while one waits. On `held`, it starts them in boot order: `[INIT] keystore HOLDS THE DEVICE KEY: THE SERVICES THAT WAITED FOR IT START`.
- **The trial boot** is confirmed without them. A user's wait for the passphrase does not roll a release back, so a release that breaks only these services is confirmed too; their logs show it.
- **Tests:**
  - `tests/runtime.rs`: the slot and the ABI;
  - the `tls` suite's `passphrase_wait_check`:
    - **the boot:** a raw disk with passlock's file in `system/keystore/`, no TPM, and a store disk;
    - **it checks** that `keystore` waits, that `blockstore`, `storefs`, `tls` and `ssh` are prepared and not running while the shell runs, and that the shell holds slot 44 with badge 6 of `keystore`'s endpoint.
- **Docs:**
  - `docs/api` (ABI 31);
  - `docs/profile/bootstrap.md`: the shell's holdings, and the wait in step 4.

**Left:**
- **The unlock, and the start after it,** in QEMU. Typing the passphrase is the shell's command (APP's part, `issues/requests-APP.md`), and nothing else holds the badge.
- **"A wrong one leaves them unstarted, with the reason logged."** `keystore` logs each refusal (308-NET-0055), and `init` starts nothing until `held`. The QEMU check of it waits for the same command.
- **A safe start** asks for the passphrase as a normal one does. Its services left out stay left out, and the keyed ones wait the same way. It is not checked in QEMU yet.

**Landed** in round 113 (`main` d8d52c57, 2026-11-10): every group of the gate passed with ABI 31, the `tls` suite's `passphrase_wait_check` among them. **Left:** the unlock and the start after it, in QEMU, once the shell's command is in (308-APP-0130).

## Related

[308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md), [308-KRN-0204](308-KRN-0204-a-store-client-that-may-recover-the-volume-key.md), [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md).
