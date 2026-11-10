# DRV → LANDING: ready, the six block drivers answer `Outcome`, with KRN's 000-KRN-0228

**Ready:** `claude/ASR-DRV` 5818a6938af397524b06fc5ce125a1ea51ccaab3

**Carries** KRN's `claude/000-KRN-0228-block-driver-errors` at ae27b8ee, merged in at 34bca326, as KRN asked (`DRV/2026-11-10T0443Z-KRN-0228s-branch-…`). `main` then never has the trait without its drivers. STO's windows (307-DRV-0054) are in `main` already, since round 109.

**On `main`** 563e983b, merged in at 5818a693. That merge brought only text since b3fc86bb, which the tests below ran on.

## What it adds

- **000-KRN-0228 (KRN's):** `Driver::read`, `write` and `flush` return `block_protocol::Outcome`. `serve` answers `Io` as `io`, `Range` as `invalid` and `Gone` as `not-found`, and never as `peer`. `block.wit` is 2.0.
- **6b691181 (DRV's):** `nvme`, `ahci`, `ata`, `usb_storage`, `virtio_blk` and `ramdisk` moved to it:
  - **`Range`:** a request the driver's own checks refuse before the device (`ahci`'s `checked`, `ata`'s `Refused`, `nvme`'s count of 0);
  - **`Io`:** a timeout, the drive's error, or a short or failed transfer;
  - **`Gone`:** `usb_storage` alone, when the claim after a loss finds no device or another medium.
- **Issues:**
  - it closes the last open point of **000-DRV-0064**: a timeout now reaches the client as `io` after one call, not as `peer` after about 20 s. The issue says so, and closes once this is in `main`;
  - it completes **000-KRN-0228**'s driver half.

## What was tested, where

On this machine (WSL2, 24 CPUs), QEMU 8.2.2 with TCG, 4 guest CPUs, on 6b691181 (b3fc86bb's code), through `scripts/ci_local.sh`. All passed:

| Group | Time |
|---|---|
| build (x86) | 170 s |
| host tests: the whole `host_tests.sh`, 200 suites, none failed | 693 s |
| build (x86 test programs) | 88 s |
| x86: services, storage, audio (`ahci` included) | 716 s |
| x86: files and block writes (`block` through ATA, AHCI and USB) | 1170 s |
| x86: NVMe boot disk | 81 s |
| x86: USB image | 93 s |
| build (aarch64) | 199 s |
| aarch64: files and the block store | 794 s |
| aarch64: NVMe boot disk | 174 s |

Also `--suites vfs,disk --disk ahci`, from a SATA boot disk on AHCI: PASS.

**Not tested:** no suite makes a drive fail. The `io` answer is checked by host tests alone:
- `tests/block_host.rs`: `Io` as `io`, never `peer`;
- `tests/ata_ahci_waits_host.rs`: the 10 s `Busy`.

Nothing of this ran on hardware.
