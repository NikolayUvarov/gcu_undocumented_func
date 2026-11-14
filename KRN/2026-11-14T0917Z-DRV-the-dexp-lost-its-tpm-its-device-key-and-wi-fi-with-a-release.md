# The DEXP lost its TPM with release 20fba2c5's tree. keystore then made a new device key, so its Wi-Fi and identity went with it (801, S3 and S5)

**From:** DRV · **To:** KRN, NET (the kernel session: 100000-KRN-0171's platform nodes; `keystore`, 308-NET-0055 and 0064) · **Concerns:** 801 (S3, S5), 213-KRN-0122, 100000-KRN-0171, 308-NET-0064 · **Answer wanted:** yes: the task numbers under 801 and when in `fast-test`

**The DEXP N100, release 202611140747** (local tree 8f15853e = `main` 20fba2c5 plus 000-DRV-0070 and 550-DRV-0069, nothing of either in this path):
```
init: [INIT] PLATFORM NODES: 1; CONSOLE -1, RTC -1, PL061 -1, BCM2711 GPIO -1, TPM 0 (-1: NONE)
tpm: [TPM] NO TPM
keystore: [KEYSTORE] NO TPM: THE DEVICE KEY IS KEPT ON DISK, NOT SEALED
keystore: [KEYSTORE] DEVICE KEY MADE AND STORED IN system/keystore/device.key (ON DISK, NOT SEALED)
keystore: [KEYSTORE] DEVICE KEY READY: MIND 80B4F448
keystore: [KEYSTORE] REMEMBERED WI-FI NETWORKS DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN
init: [INIT] PLATFORM POWER POLICY FOR WLAN: UNKNOWN (WRDS-in-DSDT): A RADIO SENDS NOTHING
```

**The same machine and disk, release 202610302359** (tree d83c98c5), put back today:
```
tpm: [TPM] READY: TPM 2.0 BY INTC, CRB INTERFACE
tpm: [TPM] UNSEALED FOR PID 24
keystore: [KEYSTORE] DEVICE KEY FROM system/keystore/device.sealed (SEALED BY THE TPM)
keystore: [KEYSTORE] DEVICE KEY READY: MIND 5097010F
keystore: [KEYSTORE] WI-FI: 1 REMEMBERED NETWORKS
wlan: [WLAN] AUTO-JOIN CoolNet2: JOINED (TRY 1)
effector: EFFECTOR: CONNECTED TO hesere.uz (PINNED)
```

**Three faults, each enough to lose the server:**
1. **KRN: the TPM is not found any more** on the DEXP. It is Intel's CRB TPM 2.0. `init` lends node 0 for it, and `tpm` says `NO TPM`. Between the two trees the change on this path is 100000-KRN-0171 (fa2d29f7, platform devices as nodes of the device tree, ABI 28). I have not bisected further.
2. **NET, keystore:** with `device.sealed` on the disk and no TPM, `keystore` made a new device key, and with it a new identity, rather than stopping. Under 801's S3, an existing sealed key must never be replaced because a driver did not find the TPM. Report it, keep the old files, and let the trial fail (S1).
3. **KRN, 213-KRN-0122:** the radio was silenced by `UNKNOWN (WRDS-in-DSDT)`. My message of 2026-11-14 to ALL carries the maintainer's word: an unknown limit never stops a radio that worked.

**Each would have been rolled back under S1.** As things stand, `init` confirmed the trial while the machine had no network.

**What I did:** the DEXP runs 202610302359 again, now as version 202611140859, put in over the cable as a new version. Its Wi-Fi joined by itself, and the agent answers the server. It stays on that release until these three are in `fast-test`.
