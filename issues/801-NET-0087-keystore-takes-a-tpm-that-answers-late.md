# 801-NET-0087 — `keystore` takes a TPM that answers late: the key it sealed held in place of the one for this boot

**Type:** network (service) · **Owner:** `NET` track, made by STO (801-NET-0079's follow-up, the maintainer's word of 2026-11-18) · **Priority:** P1 · **Status:** open (2026-11-18) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Roadmap:** stage V (remote capabilities) · **Constitution:** MC-6.6, MC-9.9, MC-11.9

## Problem

[801-NET-0079](../issues-done/801-NET-0079-keystore-keeps-the-slot-befores-keys-and-networks.done) made `keystore` take a key for this boot only when a key a TPM sealed is there and no TPM answers within 5 s, and change nothing. The boot then has no remembered networks and no secrets, the block store's volume key among them, until the next boot.

A TPM can answer late in the same boot:
- the `tpm` service found its TPM not answering and was started again;
- the service failed and `init` restarted it;
- an unseal failed once.

The boot then stays without its network although its key could be had. Under S1 its trial is rolled back, and a release that would have worked is refused (801's S5: operability first).

## Plan

- **`keystore`:**
  - while a sealed key waits for its TPM (no TPM answered at the start, or the unseal failed), the TPM is asked again every 10 s, each call bounded to 5 s, so that a `tpm` service that stopped holds nothing up;
  - once the TPM answers and unseals the key, that key is held in place of the one for this boot, and with it the networks and secrets that open under it;
  - nothing is written: the files are as they were. A TPM that answers but refuses the blob, or finds it damaged, ends the waiting and leaves the key for this boot.
- **[801-STO-0052](801-STO-0052-the-store-mounts-once-keystore-gives-its-key.md):** the block store mounts again once `keystore` gives its kept volume key.
- **The check:**
  - a test build of `keystore` (`late-tpm-test`, built by `scripts/x86_fixtures.sh`) takes no TPM in the boot's first minute;
  - the `tls` suite's TPM check boots it with TPM A, which sealed the key.

## Acceptance criteria

- **QEMU, the `tls` suite's TPM check (x86):** with the late build and TPM A:
  - a key for this boot only, and the store not mounted;
  - then `THE TPM ANSWERS NOW`, A's key, the same as before, held;
  - the store mounted again in the same boot, reading back its object;
  - no file in `system/keystore` changed or set aside.
- The `tls` suite's other checks pass as before, on x86 and aarch64.

## Related

[801](801-the-bench-stays-manageable.md) (S5), [801-NET-0079](../issues-done/801-NET-0079-keystore-keeps-the-slot-befores-keys-and-networks.done), [801-KRN-0244](801-KRN-0244-the-tpm-lent-over-pages-the-firmware-lists-as-reserved.md), [801-STO-0052](801-STO-0052-the-store-mounts-once-keystore-gives-its-key.md).
