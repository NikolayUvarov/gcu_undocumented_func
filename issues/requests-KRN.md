# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (3 requests waiting, 2026-10-10; the microcode's VFS client became [550-KRN-0061](550-KRN-0061-bcm-wifi-reads-its-microcode.md); the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md), the toolchain installed once [000-KRN-0060](000-KRN-0060-toolchain-once-before-the-parallel-build.md), `bcm_wifi` as a boot service [550-KRN-0059](../issues-done/550-KRN-0059-bcm-wifi-at-boot.done), and the tools track's `SLOT_SHELL` and `SLOT_CLIPBOARD` from its branch [211-KRN-0058](211-KRN-0058-slots-for-the-shell-and-the-clipboard.md)) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## The IDL fuzzer in CI's host tests (500-ASR-0001)

**Recorded by:** the assurance track (`ASR`), 2026-10-09, for [500-ASR-0001](500-ASR-0001-idl-decoder-fuzzing.md).

### Problem

`tests/idl_fuzz_host.rs` fuzzes every generated IDL decoder with a fixed seed: 24 receivers and 82 types, 50 000 inputs per target, about 4 s. It is not in CI's host tests yet, and the CI files are the kernel track's.

### Plan (a proposal; the kernel track decides)

- Add `idl_fuzz` to the list of host tests in `.github/workflows/ci.yml` (the step "Host tests") and in `scripts/ci_local.sh` (`host_tests`), built like the others: `rustc --edition=2021 --test tests/idl_fuzz_host.rs`.
- If 175-KRN-0046 changes how the host tests fail, the line follows that.

### Acceptance criteria

CI runs the test on every push, and a finding fails the host-test step.

## Bus mastering off at boot until a driver is granted the device (550-DRV-0022)

**Recorded by:** the drivers track (`DRV`), 2026-10-10, from the MacBook Pro's run of [550-DRV-0020](../issues-done/550-DRV-0020-bcm4331-read-only-probe.done).

### Problem

The kernel turns bus mastering on when it grants a device's resource to a driver (`pci::enable`) and off before a driver is restarted (`pci::quiesce`). It leaves every other function's command register as the firmware set it. On the MacBook Pro the firmware leaves the BCM4331 with bus mastering on and its 802.11 core running (command `0006` in `hw0001.txt`). `bcm_wifi` now holds that core in reset when it starts ([550-DRV-0022](550-DRV-0022-bcm4331-core-reset-and-sprom.md)), about 5 s into the boot. Until then, and for any device no driver is given, a device the firmware left running may write to memory. The profile declares no IOMMU (MC-1.5), so nothing else stops it.

### Plan (a proposal; the kernel track decides)

- At PCI enumeration, clear bus mastering (command bit 2) on every function except the bridges, and log the ones that had it on.
- `pci::enable` turns it on when a driver is granted the device, as now.

### Acceptance criteria

On QEMU and on the MacBook Pro, the hardware report shows bus mastering off for every function no driver was granted, and the drivers work as before.

## A failed flush is reported again by the next one (211-DRV-0019)

**Recorded by:** the drivers track (`DRV`), 2026-10-10, from the local gate of `claude/ASR-DRV` (211-DRV-0019).

### Problem

In `vfs_server/src/disk.rs`, a flush whose write fails sets `failed` and returns false. The next flush takes `failed` and returns false again, even when it writes every changed sector and the drive empties its cache.

`tests/usb_image_smoke.py` (211-KRN-0050) runs `sync` right after the boot disk is plugged in again and expects `OK`. With 211-DRV-0019's `usb_storage` it gave `ERROR: SYNC: I/O ERROR` there in all five local runs, and a second `sync` gave `OK`. On `main` the first `sync` said `OK` in all three. With debug lines added for the run (not committed):

- 15.54 s: the disk is gone;
- 19.53 s: `vfs_server` says the drive does not answer;
- 19.54 s: the journal's save flushes, and its write of LBA 1032193 (the log volume's FAT) fails in the quiet window;
- 30.99 s: the disk is back;
- 31.27 s: the test's `sync` reports that earlier failure.

In `main`'s run with the same debug lines, no flush write failed during the outage. Most likely `usb_storage`'s new log lines about the loss are what give the journal's save something to flush then.

### Plan (a proposal; the kernel track decides)

- A flush that fails reports it once and does not set `failed`: its changed sectors stay changed and the next flush writes them.
- `failed` stays for what only a later flush can report: a changed sector whose write failed when it was evicted.

### Acceptance criteria

`tests/usb_image_smoke.py` passes with 211-DRV-0019's `usb_storage` (branch `claude/ASR-DRV`): the first `sync` after the disk is back says `OK`. A flush whose own write fails still reports it.
