# 308-NET-0064 — The old secrets set aside when `keystore` makes the device key anew

**Type:** `keystore` · **Owner:** STO, by agreement with the network track's owner (the kernel session, 2026-11-10) · **Priority:** P2 · **Status:** in progress (in `main` since round 116, 2026-11-10; the `recovery_key_check` criterion left, STO's) · **Blocked by:** — · **Main task:** [308](308-encryption-at-rest.md), for [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md) and [308-APP-0122](308-APP-0122-a-recovery-key-shown-once-and-typed-back.md) · **Constitution:** MC-6.8, MC-9.6, MC-11.9

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

## Progress (2026-11-10)

**Made by STO** on `claude/relaxed-meitner-5bmhpz`, as planned.
- **`keystore/src/secrets.rs`, `set_aside`:** every file of `services/` and `secrets/` ending in `.kept` or `.new` goes to `<name>.before`, logged one by one (`SECRETS OF SERVICE blockstore MADE UNDER AN EARLIER DEVICE KEY: SET ASIDE AS system/keystore/services/blockstore.kept.before`).
  - A `.before` left by an earlier time goes first, and the line says so: it was sealed under a key older still.
  - A file that is not moved is named, and its owner's puts stay refused.
- **The programs' secrets too,** in `secrets/`, beside the services'. They are sealed under the same device key and would be refused for ever in the same way, so the plan's "every owner" covers them.
- **`keystore/src/main.rs`, `anew`:** called where the device key is made anew and kept:
  - its file missing, or damaged;
  - with a TPM, the sealed key not opened on this TPM, or damaged;
  - with a TPM and no sealed key, a key made because the plain file is missing or does not read.

  Not where a key is made for this boot only, the stored one being unreadable, since the earlier key may come back.

**Tested** in the `tls` suite on x86 and aarch64 (8 and 8 checks, the branch merged with `main` d8876cab):
- **The device key check**, after `device.key` is damaged, with the store's disk made blank:
  - the set-aside lines name `blockstore`, `netpeer` and `netcopy`;
  - the store's new volume key is kept (`SECRET volume-… OF SERVICE blockstore KEPT`, `VOLUME KEY MADE …, KEPT BY KEYSTORE`);
  - on the host, each earlier file is whole as `.before`.
- **A file copied under another owner's name**, the boot before (netcheck's as `netpeer`, the store's as `netcopy`): it opens nothing, and on the host it stays where it is and as it was, with no `.before`.
- **The TPM check:** TPM B's new key sets the store's file aside.

**Left:** the criterion on `recovery_key_check` (it removes `device.key` alone), once APP's 308-APP-0122 is in `main`.

**Landed** in round 116 (`main` 46d756be, 2026-11-10; STO's request of 1642Z), every group of the gate passed. **Left:** the criterion on `recovery_key_check`. APP's 308-APP-0122 has been in `main` since round 115, so it is STO's next step.

## Related

[308-NET-0055](308-NET-0055-keystores-passphrase-without-a-tpm.md), [308-STO-0028](308-STO-0028-a-recovery-key-and-a-passphrase.md), [308-APP-0122](308-APP-0122-a-recovery-key-shown-once-and-typed-back.md), [308-STO-0022](../issues-done/308-STO-0022-the-volume-key-held-by-keystore.done).
