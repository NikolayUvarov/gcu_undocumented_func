# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (6 requests waiting, 2026-10-10; the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md)) · **Recorded by:** the tools track (APP), 2026-10-06

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

## The pinned toolchain installed once before the parallel build (000-KRN-0020)

**Recorded by:** the assurance and drivers session (`ASR`, `DRV`), 2026-10-09, after the maintainer's build of `fast-test` failed.

### Problem

`rust-toolchain.toml` gained `components = ["rust-src"]` (e5a34c2, 250-APP-0020). On a machine with `nightly-2026-10-02` but without that component, `02_build.sh` starts every crate's cargo at once (000-KRN-0020). Each cargo asks rustup to add `rust-src`, and the downloads race on one file:

```
error: component download failed for rust-src: could not rename 'downloaded' file from
'~/.rustup/downloads/7da4d…partial' to '…': No such file or directory (os error 2)
```

All 70 crates fail, and the build reports them as failures of the code. A single `rustup component add rust-src --toolchain nightly-2026-10-02` fixed it.

### Plan (a proposal; the kernel track decides)

- Before the parallel step, `02_build.sh` runs `rustup toolchain install` once in the repository. It reads `rust-toolchain.toml` and installs the channel, the components and the targets. A failure stops the build with that message.
- `01_prepare_env.sh` does the same, so the two cannot disagree.

### Acceptance criteria

On a machine whose rustup lacks a component the toolchain file names, `02_build.sh` installs it once and the build succeeds. A failed install is reported as such, not as 70 failed crates.

## `bcm_wifi` as a boot service (550-DRV-0020)

**Recorded by:** the drivers track (`DRV`), 2026-10-09, for [550-DRV-0020](550-DRV-0020-bcm4331-read-only-probe.md), stage 1 of the MacBook Pro's Wi-Fi ([550-DRV-0006](550-DRV-0006-broadcom-wifi.md)). The maintainer put Wi-Fi first among the network tasks.

### Problem

`bcm_wifi/` is the driver for the MacBook Pro's Broadcom BCM4331 (`14E4:4331`, class `028000`). Its stage 1 is written and builds, and only reads the chip. Three things in the kernel track's files keep it from running:

- it is not in `BOOT_SERVICES`/`BOOT_FILES`;
- `init` does not start it;
- `02_build.sh` does not build it.

`BOOT_IMAGES` is 32 and sizes `BootInfo.programs`, so one more boot image is an ABI change.

### Plan (a proposal; the kernel track decides)

- **`common/abi.rs`:** `bcm_wifi` and `bcm_wifi.elf` in the boot lists, `BOOT_IMAGES` one larger, with the ABI version and its transition as the track does them.
- **`02_build.sh`:** `"bcm_wifi:bcm_wifi:bcm_wifi.elf"` in the crate list (x86 only; there is no such chip on the aarch64 targets).
- **`init`:**
  - start `bcm_wifi` when `DEVICE_FIND` finds vendor `14E4` device `4331`, or class `02:80:00` from vendor `14E4`; otherwise `bcm_wifi NOT STARTED: NO DEVICE`, as for the other drivers;
  - grant BAR0 (16 KiB MMIO) in `SLOT_DEV0`. Stage 1 needs nothing more.
- **Later stages, for the same grant list when they come** (requests then):
  - the MSI (or INTx) line;
  - a DMA region for the transmit and receive rings (about 256 KiB);
  - a read-only `vfs` client for the microcode file (`firmware/` on the boot volume, put there by the maintainer's build);
  - a service endpoint for `NET`'s station.
- **A configuration write limited to the BCMA window registers** (0x80, 0xAC, 0x84) of the driver's own function may be asked for later, if moving BAR0's windows turns out to be needed. Stage 1 does not move them.

### Acceptance criteria

On the MacBook Pro, `init` starts `bcm_wifi` with BAR0, and its stage-1 lines are in the boot log. On QEMU, which has no such chip, it is not started and says so.

**Seen on the MacBook Pro (2026-10-10):** the kernel session's `fast-test` a00618b started `bcm_wifi` with BAR0, and its stage-1 lines are in the boot log ([550-DRV-0020](../issues-done/550-DRV-0020-bcm4331-read-only-probe.done)). The acceptance criteria hold there; the request waits only for this to reach `main`.

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

## `bcm_wifi` reads its microcode: a read-only VFS client (550-DRV-0023)

**Recorded by:** the drivers track (`DRV`), 2026-10-10, for [550-DRV-0023](550-DRV-0023-bcm4331-microcode-runs.md), stage 2 of the MacBook Pro's Wi-Fi.

### Problem

Stage 2 of `bcm_wifi` loads Broadcom's microcode into the BCM4331's 802.11 core. The maintainer's build copies it onto the written disk under `data/firmware/b43/` (AGENTS.md section 3, `scripts/proprietary.sh`), never into the image. `init` gives `bcm_wifi` BAR0 only (550-KRN-0059 on `fast-test`), so it cannot read the file.

### Plan (a proposal; the kernel track decides)

In `init`'s `bcm_wifi` arm, the same read-only client `gpio` gets for `hwdocs/`:

```rust
grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_READER)?, CLIENT);
```

`vfs_server` starts before `bcm_wifi` (PIDs 12 and 18 on the MacBook Pro). Nothing else is needed for stage 2: the microcode goes through the core's registers, without DMA or an interrupt.

### Acceptance criteria

On the MacBook Pro, `bcm_wifi` opens `data/firmware/b43/ucode29_mimo.fw` and logs its size. Without the file it logs that it is missing and runs on. It cannot write anywhere or open a private directory.

