# 351-KRN-0022 — The updater's authorities: request flags and init's grants

**Type:** kernel (ABI) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0007](351-UPD-0007-updater-service.md) (the updater, which asks for them) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-3.11, MC-9.3

## Problem

[351-KRN-0014](../issues-done/351-KRN-0014-trial-boot-and-confirmation.done) gave the system its slot, the trial and init's confirmation. It left out the grants the updater needs, because no updater exists yet to ask for them:

- TLS, for the release download;
- the update zone of `vfs_server` ([351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md));
- a network grant;
- restarting the machine through `init`.

## Plan

- **Request flags** in `common/abi.rs` and `libmind::process` for each authority. They are additive, so the ABI version does not change: an older kernel refuses an unknown flag.
- **`init`** grants them to the `updater` service only, from its own capabilities.

## Acceptance criteria

The updater gets exactly these four authorities and no other, and a program that asks for any of them is refused (an `isolation` case).

## Related

[351-UPD-0007](351-UPD-0007-updater-service.md), [351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md).
