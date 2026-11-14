# 801-NET-0079 — `keystore` keeps the slot before's device key, networks and secrets: nothing replaced on trial, nor while no TPM answers

**Type:** network (service) · **Owner:** `NET` track, made by STO (801's "keystore's owner", `ALL/2026-11-14T0906Z-STO-…` on `internal-dev-messages`) · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Roadmap:** stage V (remote capabilities) · **Constitution:** MC-9.4, MC-9.9, MC-11.9, MC-12.1

## Problem

On 2026-11-14 the DEXP's release 202611140747 logged `[KEYSTORE] REMEMBERED WI-FI NETWORKS DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN`. Its `keystore` held a device key other than the one `wifi.kept` was written under. On 202610302359 the same machine had joined CoolNet2 by itself (801).

Today's `keystore` makes a new device key, stores it and sets the owners' secrets aside in several cases:
- **A key sealed by a TPM while no TPM answers** at its start. The TPM service may be absent or late, or a release may not reach the TPM. `keystore` then takes the path without a TPM. It finds no `device.key`, which was removed when the key was sealed, and makes and stores a new key.
- **A sealed key the TPM refuses, or a damaged key file,** on any boot, trial or not.
- **A key file it cannot read.** The log says "for this boot", yet the new key was stored over the old one.
- **On a boot on trial,** it also converts the slot before's files: it seals an unencrypted key and removes the copy, and it removes the copy after an unlock.

Each case changes what the slot before reads (801's S3). A rollback then finds a key that is not its own, networks that do not open and secrets set aside, among them the block store's volume key. The DEXP's log does not say which case it was.

## Plan

- **A sealed key while no TPM answers:** the TPM is waited for 5 s at most. If none answers, a key for this boot only is used and nothing in `system/keystore` changes, on any boot.
- **A key for this boot only** (no TPM, an unreadable file, a store that fails): never stored. No secret and no network is written under it: secrets calls answer `no-key`, and a remembered network holds for this boot only.
- **On a boot on trial** (`boot_slot.trial`, from the boot information every program gets), nothing the slot before reads is written, moved or removed:
  - a key that is missing, refused or damaged is not replaced; a key for this boot only is used;
  - sealing an unencrypted key, removing the unencrypted copy and setting secrets aside wait for a boot not on trial;
  - `passphrase-set` is refused;
  - networks remembered or forgotten go to `wifi.trial`, under a key derived from the device key and the trial's manifest digest, so only that release opens it. Booted not on trial, the release writes it as `wifi.kept` and removes it.
- **Unchanged:** every key and file format, and what a boot not on trial does with a key that opens.
- **Off trial, a key or networks file that does not open is set aside,** as `<file>.before`, before a new key is stored. It is never written over: it is the evidence, and may be the only copy (KRN's point, from its withdrawn version 5a44ca6c). If it cannot be moved, the key is for this boot only.

## Acceptance criteria

- **QEMU, the `tls` suite's TPM check (x86):** after the key is sealed by TPM A, a boot without a TPM:
  - makes and stores no key and changes no file in `system/keystore`;
  - sets no secret aside;
  - leaves the store disk unmounted, its secret not given under the key for this boot.

  TPM A back: the same device key, and the store reads back its object.
- **QEMU, the `update` suite (x86):** the trial boot of release 6 says it changes nothing, and holds the device key of the boot before.
- **QEMU, the `boot` suite's trial check (x86):** slot B on trial with a damaged `device.key` leaves it byte for byte, and makes no `.before`. The device key check and the TPM check find the damaged key, and TPM A's refused blob, set aside whole.
- **Not shown here:** a trial that cannot open the key being rolled back. That is S1 (UPD and KRN).

## Progress (2026-11-14)

**Built** on `claude/relaxed-meitner-5bmhpz`:
- a93835838, the rules;
- b2ee66b6, the setting aside;
- bc286b95, the trial check's boot with RDRAND and its writes reaching the image.

It is in `fast-test` (8fd84ea1).

**DRV's log of the DEXP** (`KRN/2026-11-14T0917Z-DRV-…`) shows the first case of the Problem:
- `init` found no TPM node, and `tpm` said `NO TPM`;
- `keystore` then made and stored a new key, set the secrets aside, and the networks did not open.

Under this task, that boot waits 5 s for the TPM. It then uses a key for this boot only and changes nothing.

**Tested** in this session's container (4 CPUs, QEMU 8.2.2 under TCG, x86):
- **the `tls` suite on b2ee66b6:** 13 checks passed, among them:
  - the TPM check's boot without a TPM after A sealed the key;
  - A then opening the same key and store;
  - the `.before` copies of the damaged key and of A's blob;
- **the `update` suite on b2ee66b6:** 4 checks passed. Release 6's trial boot says it changes nothing and holds the key of the boot before;
- **the `boot` suite on bc286b95:**
  - the trial check passed: the damaged key was left byte for byte;
  - the suite then stopped at `bar_move_check`, KRN's MSI storm of 000-KRN-0077, which `keystore` does not touch; it is being run again.

## Related

[801](801-the-bench-stays-manageable.md) (S3, S5), [351-NET-0006](../issues-done/351-NET-0006-device-key-sealed-by-a-tpm.done), [308-NET-0055](../issues-done/308-NET-0055-keystores-passphrase-without-a-tpm.done), [308-NET-0064](../issues-done/308-NET-0064-the-old-secrets-set-aside-when-the-device-key-is-made-anew.done), [550-NET-0024](../issues-done/550-NET-0024-remembered-wi-fi-networks.done).
