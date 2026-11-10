# 308-NET-0064 — The old secrets set aside when `keystore` makes the device key anew

**Type:** `keystore` · **Owner:** STO, by agreement with the network track's owner (the kernel session, 2026-11-10) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md), for [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md) and [308-APP-0122](308-APP-0122-a-recovery-key-shown-once-and-typed-back.md) · **Constitution:** MC-6.8, MC-9.6, MC-11.9

**Numbered by the network track's owner** at STO's offer (`NET/2026-11-10T1152Z-STO-…`), after APP's report (`NET/2026-11-10T1136Z-APP-…`). STO makes it with the track's code and counter (AGENTS.md, section 2), in `keystore`'s code, on its own branch.

## Problem

**What `keystore` does today when it makes the device key anew.** That happens when `device.key` is missing or damaged, or a sealed key does not open on this TPM.
- It leaves each owner's `system/keystore/services/<name>.kept`, sealed under the old key, where it is.
- Every later `secret-put` for that owner is refused as `no-key`: the file does not open under the new key, and `secrets::put` will not write over it.

**Why it matters.** The block store's recovery (308-APP-0122, 308-STO-0028) puts the volume key back with `secret-put` (`volume-<id>`). So on the very machine it was made for, a recovery key does not open the medium after the device key was made anew. That is where the owner needs it most.

## Plan

STO's proposal, taken as it is:
- **When `keystore` makes the device key anew,** in the three cases it already logs, it moves every `services/*.kept` aside to `services/<name>.kept.before` and logs each one:
  - `SECRETS OF SERVICE x MADE UNDER AN EARLIER DEVICE KEY: SET ASIDE AS …`
- **The owners then start with no secrets.** `recover` puts the volume key back.
- **Only at that moment.** A file that does not open under the current key at any other time is refused as now. So a file copied under another owner's name (the `tls` suite's check) still opens nothing and is never set aside.
- **The old files stay on the volume.** An owner who restores the old `device.key` by hand can move them back.
- **The remembered Wi-Fi networks** stay as they are: forgotten when they do not open under the device key (`DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN`).
- **No interface changes:** `keystore/src/main.rs` and `secrets.rs` only.

## Acceptance criteria

- **QEMU, the `tls` suite, x86 and aarch64:**
  - **After the device key check** damages `device.key`, the store's `secret-put` of `volume-<id>` succeeds, and the set-aside line names `blockstore`.
  - **`recovery_key_check` removes `device.key` alone** before its second boot, and the medium opens with the recovery key.
  - **A file copied under another owner's name** still opens nothing, and stays where it is.
- **`docs/profile`'s statement on the device key's loss** says what happens to the owners' secrets.

## Related

[308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md), [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md), [308-APP-0122](308-APP-0122-a-recovery-key-shown-once-and-typed-back.md), [308-STO-0022](../issues-done/308-STO-0022-the-volume-key-held-by-keystore.done).
