# Effector handoff: the kernel track's parts are numbered 750-KRN-0119 to 0121; no build yet

**From:** KRN · **To:** HUMAN · **Written:** 2026-10-20 13:45 UTC · **Answers:** ALL/2026-10-15T2016Z-HUMAN-effector-native-agent-and-package-handoff.md (and beside HUMAN/2026-10-20T0548Z-UPD-effector-handoff-upd-parts-numbered.md) · **Concerns:** 750, `issues/requests-EFF.md`, 750-KRN-0119, 0120, 0121; 750-UPD-0019, 0020; `main` 825b563 · **Answer wanted:** yes (who takes `EFF`; whether the agent may be on by default in a test image)

## What the kernel track owns, and its tasks

The kernel track (`KRN`) owns `init`'s grants and the ABI's list of boot images. Its parts of the handoff are numbered on `main`:
- **[750-KRN-0119](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/750-KRN-0119-boot-images-and-grants-for-effector.md),** boot images and `init`'s grants for `effector` and `effector_gw`:
  - the two services are off unless the owner enables them (173-KRN-0035);
  - `effector` gets the network and TLS clients only;
  - `effector_gw` gets the scoped `vfs` client, the updater's client (750-UPD-0020) and `init`'s lifecycle client.
- **[750-KRN-0120](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/750-KRN-0120-writable-scopes-for-a-service.md),** writable scopes for a service: a `vfs_server` client confined to one directory, such as `data/apps/`. The link track's `link` needs the same thing for `data/link/` (800-KRN-0128).
- **[750-KRN-0121](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/750-KRN-0121-a-lifecycle-client-for-named-services.md),** a lifecycle client limited to named services, so that `effector_gw` can restart what the owner's policy names and nothing else.

## The contract

The package and signature contract is the update track's, draft 0.1 in 750-UPD-0019. The kernel track's parts depend on it in two places, and add nothing to it:
- the loader's check of an installed application, which the update track's shared check provides;
- the badge the gateway's updater client carries.

## The first build

There is none. 0119 is blocked by the two services, which exist only as issues (750-EFF-0006, 0007), and by 173-KRN-0035. 0120 can start now, and the kernel track takes it next among these, since the link track needs it too. It would change the ABI with 351-KRN-0115 and 0116 (ABI 12).

## Asked

- **Who takes `EFF`** (the agent and the gateway). The kernel track does not take it beside its own work.
- **Whether a test image may start the agent without `data/services.txt`** before 173-KRN-0035 is done. That would let a test against the Effector server begin earlier; the real image keeps it off.
