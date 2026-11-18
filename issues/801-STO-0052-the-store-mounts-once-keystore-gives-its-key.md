# 801-STO-0052 — The block store mounts its medium once `keystore` can give its kept volume key

**Type:** storage (service) · **Owner:** `STO` track · **Priority:** P1 · **Status:** open (2026-11-18) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) · **Roadmap:** track B · **Constitution:** MC-4.8, MC-6.6

## Problem

A medium whose volume key `keystore` keeps (308-STO-0022) is mounted once, at the store's start. If `keystore` holds no device key it may use then, the medium is not mounted until the next boot, though the key may come in the same boot:
- a key for this boot only, while no TPM answers (801-NET-0079);
- the TPM answering later (801-NET-0087).

## Plan

- **`blockstore`:** when `keystore` answers `no-key` to the volume key's `secret-get`:
  - the log says so: `… WHICH HOLDS NO DEVICE KEY IT MAY USE YET: TRIED AGAIN EVERY 30 S`;
  - the medium is mounted again every 30 s until the answer is another;
  - once mounted, the copy on a second medium is mounted beside it again.
- **Unchanged:** a secret `keystore` does not keep, or a key that does not open the check, still leaves the medium unmounted until a recovery.

## Acceptance criteria

- **QEMU, the `tls` suite's TPM check (x86),** with 801-NET-0087's late build: the store not mounted, then `KEYSTORE GAVE THE KEPT VOLUME KEY NOW: THE MEDIUM MOUNTED` and its object read back, in the same boot.
- The `store` suite passes as before.

## Related

[801-NET-0087](801-NET-0087-keystore-takes-a-tpm-that-answers-late.md), [308-STO-0022](../issues-done/308-STO-0022-the-volume-key-held-by-keystore.done), docs/storage ("The volume key kept by keystore").
