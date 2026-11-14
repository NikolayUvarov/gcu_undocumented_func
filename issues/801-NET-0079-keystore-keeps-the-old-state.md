# 801-NET-0079 — `keystore` keeps the old state: no device key or networks file it cannot open is written over, set aside on a trial boot, or removed

**Type:** network (`keystore`) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S3) · **Roadmap:** stage V · **Constitution:** MC-9.4, MC-11.9, MC-12.1

## Problem

**The DEXP lost its network on 2026-11-14** with release 202611140747: `[KEYSTORE] REMEMBERED WI-FI NETWORKS DO NOT OPEN UNDER THIS DEVICE KEY: FORGOTTEN`. Its radio was not silenced: `rtw88_wifi` is given no platform word and reads none (801-KRN-0243). The boot had a device key other than the one the networks were wrapped under, so `wlan` had nothing to join.

`keystore` replaced the old state in several ways, each before any confirmation and on a trial boot as on any other:
- **A key that does not open is written over.** A stored `device.key` found damaged, and with a TPM a `device.sealed` the TPM refuses or finds damaged: a new key is made and written in the old one's place. The previous slot then finds the new key too, so a rollback cannot bring the networks or the identity back.
- **"For this boot only" was not.** A key file that could not be read gave a key made by `made`, which stores it, so it was written over as well.
- **The unencrypted copy is removed** when a TPM first seals the key, and when a passphrase seals it (308-NET-0055), on a trial boot too.
- **The networks file that does not open** is replaced at the next change (`remember`, `forget`), and one that could not be read is replaced the same way.

The maintainer (801, S3): «старое состояние не трогается - это разве не очевидно?»

## Plan

- **On a trial boot** (`BootInfo.boot_slot.trial`), `keystore` replaces, sets aside and removes nothing in `system/keystore`.
  - A key it cannot open, or none stored, gives a key for this boot only, with nothing written: `… : A DEVICE KEY FOR THIS BOOT ONLY, NOTHING WRITTEN`.
  - A trial that needs the stored key cannot connect, fails S1 (801-KRN-0242), and its slot falls back; the previous slot finds every file as it was.
  - Sealing the unencrypted key, and removing that copy, wait for a boot that is not on trial.
- **Off trial,** a key that does not open is never written over.
  - It is set aside to `<file>.before` (one set aside before goes, as 308-NET-0064 does for the owners' secrets), then a new key is made and stored.
  - If the rename fails, the key is for this boot only.
  - A key file that cannot be read gives a key for this boot only.
- **The remembered networks:**
  - a file that does not open under this key, or is damaged, is set aside to `wifi.kept.before` off trial, so that the key it opens under brings it back;
  - on a trial it stays where it is;
  - while a file that did not open stays (a trial, or a file that could not be read), nothing is written over it: `remember` and `forget` answer `no-key` and say why.

## Acceptance criteria

- **The `boot` suite's trial check:** a damaged `device.key` on the disk slot B boots from on trial. `keystore` says the boot is on trial and takes a key for that boot alone; after the boot, `device.key` is as it was and there is no `device.key.before`.
- **The `tls` suite's device key check:** a damaged `device.key` off trial is set aside whole as `device.key.before`, and a new one is stored.
- **The `tls` suite's TPM check:** TPM A's `device.sealed`, refused by TPM B, is set aside as `device.sealed.before` (`MINDSEL1`), and B's key is sealed.
- Both arches' "network, TLS and Effector" groups and "x86: boot, display, shell, memory, clock" pass on the maintainer's machines, then in a gate.

## Related

[801](801-the-bench-stays-manageable.md), [308-NET-0064](../issues-done/308-NET-0064-the-old-secrets-set-aside-when-the-device-key-is-made-anew.done) (the owners' secrets set aside), [351-NET-0005](../issues-done/351-NET-0005-persistent-device-key.done), [351-NET-0006](../issues-done/351-NET-0006-device-key-sealed-by-a-tpm.done), [550-NET-0024](../issues-done/550-NET-0024-remembered-wi-fi-networks.done), 801-KRN-0242 (S1).
