# 308-KRN-0204 — A store client that may recover the volume key, held by the shell alone

**Type:** `init`, the ABI (a request bit) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** STO's `blockstore.wit` 2.4 calls and `BADGE_RECOVER` in `mind::blockstore` · **Main task:** [308](308-encryption-at-rest.md), for [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md) · **Constitution:** MC-3.1, MC-6.8, MC-9.6, MC-11.9

**Asked by:** STO, 2026-10-30 and 2026-11-04 (`issues/requests-KRN.md`, taken out of it with this issue). STO asked for the shell and `wm`. APP's program `recovery` (308-APP-0122) has a window of its own, so `wm` needs no copy (`KRN/2026-11-04T0441Z-APP-…`). The client goes to the shell alone, the least authority that serves (MC-3.1).

## Problem

**What the store has.** `blockstore` keeps the volume key sealed under a recovery key as well (308-STO-0028, in `main`). Its calls `recovery-new`, `recovery-keep`, `recovery-forget` and `recover` answer only a client badged `BADGE_RECOVER` (`blockstore.wit` 2.4, STO's).

**What is missing.** Nothing holds such a client, so the user cannot make a recovery key or open a store with one.

## Plan

1. **`init`** mints a client of the store's endpoint badged `BADGE_RECOVER` and grants it to the shell alone, in a slot of its own.
2. **A request bit, `REQUEST_RECOVER`,** as `REQUEST_SSH` (351-KRN-0115): a program that asks for it gets a copy from the shell, and no other program does. The shell asks the user each time, as for `REQUEST_FIRMWARE` (APP's part, 308-APP-0122). That is an ABI step, a new version with its explicit refusal of the old one (MC-12.4).
3. **Tests:**
   - `tests/runtime.rs` and `tests/boot_abi_host.rs`, for the ABI step;
   - QEMU: `stat caps` of the shell shows the client and that of `wm` does not; a program with the bit gets a copy, one without gets none, and the store refuses a recovery call from any other badge.

## Acceptance criteria

- The shell alone holds the client, and only programs with `REQUEST_RECOVER` get a copy, in QEMU on x86 and aarch64.
- The store's recovery calls answer that client alone.

## Related

[308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md), [308-KRN-0205](308-KRN-0205-the-boot-waits-for-keystores-passphrase.md), [308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md), [308-APP-0122](308-APP-0122-a-recovery-key-shown-once-and-typed-back.md), 351-KRN-0115 (`REQUEST_SSH`).
