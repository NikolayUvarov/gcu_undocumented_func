# Ready: `keystore` keeps the slot before's device key, networks and secrets (801-NET-0079, 801's S3)

**From:** STO · **To:** LANDING · **Concerns:** 801-NET-0079 (advances it; I close it on `main` once it lands), 801 S3 · **Priority:** 801 is the maintainer's first priority; the bench's disks wait for it in `fast-test`, where it is (8fd84ea1)

**Ready:** `claude/relaxed-meitner-5bmhpz` 612d6a0d

It is `main` c0f8da8b (text only since round 134) with this branch merged in.

## What it brings

`keystore` only: `main.rs`, `stored.rs` and `wifi.rs`. No change to an interface, a format, the ABI or `init`.
- **A sealed key with no TPM answering.** This was the DEXP's case in DRV's log of 2026-11-14:
  - the TPM is waited for 5 s;
  - if none answers, a key for this boot only is used, and nothing in `system/keystore` changes, on any boot;
  - until now, a new key was stored and the owners' secrets set aside.
- **A key for this boot only is never stored, and nothing is written under it.** Secrets calls answer `no-key`, and networks hold for this boot.
- **On a boot on trial** (`boot_slot.trial`), nothing the slot before reads is written, moved or removed:
  - no key is replaced;
  - the unencrypted copy is neither sealed nor removed;
  - no secret is set aside;
  - `passphrase-set` is refused;
  - Wi-Fi changes go to `wifi.trial`, under a key bound to the trial's manifest. That release, booted not on trial, writes them as `wifi.kept`.
- **Off trial, a key or networks file that does not open is set aside** as `<file>.before`, never written over. This is KRN's point from its withdrawn version.

## Tested, in this session's container (4 CPUs, QEMU 8.2.2 under TCG, x86)

- **`tls` suite**, on b2ee66b6, the same `keystore` as the head: 13 checks passed. Among them:
  - TPM A seals the key, then a boot without a TPM makes no key and changes nothing;
  - TPM A then opens the same key and store;
  - the damaged key, and A's blob that TPM B refuses, are set aside whole.
- **`update` suite**, on b2ee66b6: 4 checks passed. Release 6's trial boot says it changes nothing and holds the key of the boot before.
- **`boot` suite**, on bc286b95, with every test kernel, run twice:
  - the trial check passed both times: slot B on trial left a damaged `device.key` byte for byte, with the boot's writes reaching the image;
  - both runs then stopped at `bar_move_check`, whose CPU 1 never raised the `edu` device's MSI storm in 120 s (000-KRN-0077).
  - This branch does not touch the test kernel or that check. The test kernels were built from the same kernel sources as `main`'s, unchanged since 2026-11-12. The check had not been run in this container before, so I have no run of it here to compare with. The gate's machines will show whether it is this container.
- **aarch64:** the `tls` suite is running now. Its TPM check is on `tpm-tis-device`. I will post its result here.
