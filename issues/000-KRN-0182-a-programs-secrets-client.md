# 000-KRN-0182 — A program's client of its own secrets: `SLOT_SECRETS` from the loader (ABI 24)

**Type:** kernel (ABI, `loader`, `init`) · **Owner:** `KRN` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** — (for [000-NET-0045](000-NET-0045-secrets-per-program-in-keystore.md)) · **Roadmap:** track C (communications) · **Constitution:** MC-3.1, MC-3.7, MC-10.3, MC-11.9, MC-12.4

Recorded by the kernel track on 2026-10-28, for the network track's [000-NET-0045](000-NET-0045-secrets-per-program-in-keystore.md): `keystore` can keep a program's secrets only if it knows which program asks, by something the program cannot choose.

## Problem

- **A program has no identity at `keystore`.** A badge is set by whoever mints the client, and a program names itself however it likes.
- **The loader already gives one grant by a name** ([000-KRN-0170](000-KRN-0170-a-programs-own-data-directory.md)): a program run by its name from the booted slot, the volume's root or `data/apps/` gets its own directory. A program run by any other path gets none.
- **Secrets need the same rule,** through a client that `keystore` mints and the loader lends.

## What was done

- **`common/abi.rs`:**
  - `SLOT_SECRETS` is 40, a fixed slot no launcher fills (not in `LAUNCH_SLOTS`);
  - `SLOT_DYNAMIC` moves to 41;
  - **ABI 24.** 23 is never used: a build of `fast-test` carried it for a day with another layout (the boot image `bt_hci`, [700-KRN-0176](700-KRN-0176-bt-hci-boot-image-and-grant.md)).
- **`libmind`:**
  - `REQUEST_SECRETS` (bit 26), what a program asks;
  - `mind::network::BADGE_KEY_LOADER` (4), the loader's badge at `keystore`.
- **`init`** gives the loader a `keystore` client badged `BADGE_KEY_LOADER` in its slot 8. That client may only ask for, name and adopt a program's secrets client; it reads no secret.
- **The loader**, for a program that asks for `REQUEST_SECRETS` and runs by its name (the rule of 0170):
  1. asks `keystore` for a client (`keystore.wit` 1.4 `secrets-client`);
  2. names it for the task (`secrets-name`);
  3. moves it into the child's `SLOT_SECRETS`;
  4. after the spawn, names the task (`secrets-adopt`).

  Without `keystore`'s answer, the program starts without the client and the log says why.
- **Docs:** `docs/api` (ABI 24, the slot), `docs/profile/bootstrap.md` (the loader's slot 8), the profile's MC-3.7 row (the grants that follow a name).
- **Host test** `tests/runtime.rs`: the slot is fixed and named once, outside `LAUNCH_SLOTS`, with ABI 24.

## Acceptance criteria

- **Builds:** x86 and aarch64.
- **Host tests:** `tests/runtime.rs`.
- **QEMU, x86 and aarch64, the `tls` suite** (000-NET-0045's check): a program run by its name gets the client and keeps and reads its secret; the same code run by a path gets none.
- **The other suites** pass with ABI 24.

## Related

[000-NET-0045](000-NET-0045-secrets-per-program-in-keystore.md), [000-KRN-0170](000-KRN-0170-a-programs-own-data-directory.md), [700-KRN-0176](700-KRN-0176-bt-hci-boot-image-and-grant.md) (comes back after this with ABI 25 and handoff 25).
