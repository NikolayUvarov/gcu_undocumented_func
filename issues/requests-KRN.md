# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (5 requests waiting, 2026-10-10; QEMU's vvfat crash in the aarch64 boot suite became 000-KRN-0067; the updater's badged VFS client is done in [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done), a flush's stale failure in [211-KRN-0068](../issues-done/211-KRN-0068-a-flush-after-a-failed-one.done); the microcode's VFS client became [550-KRN-0061](550-KRN-0061-bcm-wifi-reads-its-microcode.md); the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md), the toolchain installed once [000-KRN-0060](000-KRN-0060-toolchain-once-before-the-parallel-build.md), `bcm_wifi` as a boot service [550-KRN-0059](../issues-done/550-KRN-0059-bcm-wifi-at-boot.done), and the tools track's `SLOT_SHELL` and `SLOT_CLIPBOARD` from its branch [211-KRN-0058](211-KRN-0058-slots-for-the-shell-and-the-clipboard.md)) · **Recorded by:** the tools track (APP), 2026-10-06

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

The kernel turns bus mastering on when it grants a device's resource to a driver (`pci::enable`) and off before a driver is restarted (`pci::quiesce`). It leaves every other function's command register as the firmware set it. On the MacBook Pro the firmware leaves the BCM4331 with bus mastering on and its 802.11 core running (command `0006` in `hw0001.txt`). `bcm_wifi` now holds that core in reset when it starts ([550-DRV-0022](../issues-done/550-DRV-0022-bcm4331-core-reset-and-sprom.done)), about 5 s into the boot. Until then, and for any device no driver is given, a device the firmware left running may write to memory. The profile declares no IOMMU (MC-1.5), so nothing else stops it.

### Plan (a proposal; the kernel track decides)

- At PCI enumeration, clear bus mastering (command bit 2) on every function except the bridges, and log the ones that had it on.
- `pci::enable` turns it on when a driver is granted the device, as now.

### Acceptance criteria

On QEMU and on the MacBook Pro, the hardware report shows bus mastering off for every function no driver was granted, and the drivers work as before.

## A memory quota for `blockstore` that fits its disk's index

**Recorded by:** the storage session, 2026-10-09, for [251-STO-0013](../issues-done/251-STO-0013-an-index-that-grows-with-the-medium.done) (the speech models of 251-STO-0010).

### Problem

The block store's index now grows with its medium: 56 bytes a slot, room for a block per 8 sectors (`slots_for` in `blockstore/src/store.rs`). `blockstore` allocates the slots at mount and halves them until its memory quota allows.

Its quota is the default 16 MiB (`HEAP_MAX_BYTES`), so the index stays under about 14 MiB, roughly 230 000 blocks of 16 KiB: 3.5 GiB of objects. A model disk of several such models, or a store disk larger than that, would mount with an index too small to hold every block, and mounting refuses such a store whole.

### Plan (a proposal; the kernel track decides)

- In `init`'s quotas, `"blockstore" => Quota { memory_mib: BLOCKSTORE_MEMORY_MIB, ..Quota::default() }`, next to `windows` and `compositor`, with 64 MiB. That is an index of 2^20 slots (56 MiB) and the rest of what it holds now.
- Or a quota computed from the store disk's size, if `init` knows it when it starts `blockstore`.

### Acceptance criteria

On a store disk of 8 GiB, `[BLOCKSTORE] INDEX:` reports the slots `slots_for` asks for, not a halved number.

## Free clusters counted a FAT sector at a time

**Recorded by:** the storage session, 2026-10-10, from its local gate (the branch at 68d3b80 merged with `main` at 8f05728), for the `disks` check of 251-KRN-0031 and [251-STO-0014](../issues-done/251-STO-0014-importing-a-model-disk.done).

### Problem

`Volume::free_clusters` (`vfs_server/src/fat.rs`) calls `fat(cluster)` once per cluster, and each call reads its FAT sector through the cache: a zeroed 512-byte buffer, a search of 64 tags and a copy. A model disk's FAT32 volume of 256 MiB has 65 527 clusters, so the first `df` after it is mounted does that 65 527 times.

Under TCG on aarch64 this took 2.2 s with `main`'s `vfs_server` and 9.0 to 9.6 s with the branch's, whose only difference in the call path is that a function was added elsewhere (`Volume::overwrite`). Every function on the path has the same size in both builds, so the time follows where the code lands. Over 8 s the `disks` check's `df` timed out in two groups ("aarch64: files, network and TLS", "aarch64: NVMe boot disk"). The second `df`, from the counted value, took 0.3 s. The branch gives that `df` 60 s for now.

### Plan (a proposal; the kernel track decides)

Count the free entries of each FAT sector read once: 512 sectors for that volume instead of 65 527 calls. A host test in `tests/fat_host.rs` can compare the count with the per-cluster one on FAT12, 16 and 32.

### Acceptance criteria

The first `df` of a 256 MiB FAT32 volume takes well under a second on aarch64 under TCG, and the counts match the per-cluster ones in the host test.

## A badge set only by a holder with the right to set it

**Recorded by:** the update track (`UPD`), 2026-10-10, from [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done) and [351-UPD-0008](../issues-done/351-UPD-0008-update-zone.done).

### Problem

`CAP_MINT` sets a badge on any unbadged endpoint capability, whoever holds it (`mint` in `kernel/src/scheduler.rs`, MC-3.4). A server that grants authority by badge is therefore only as safe as `init`'s care never to hand out an unbadged client of it: 351-KRN-0022 badged the clients of `init` and `vfs_server` one by one (`BADGE_LIFECYCLE`, `BADGE_READER`) for this reason. Every other badge-checking server depends on the same care, and a new grant can break it silently. Which clients are unbadged today was sent to the maintainer and the kernel session privately, as [SECURITY.md](../SECURITY.md) asks.

### Plan (a proposal; the kernel track decides)

- A right on endpoint capabilities, for example `CAP_BADGE`, that `CAP_MINT` requires to set a badge. A child never carries it unless the mask keeps it, and a badged child never does.
- `init` keeps it on the clients it badges from, and every capability it hands out lacks it.
- A new ABI version and an explicit transition (MC-12.4, 12.7).

### Acceptance criteria

A new `isolation` case: a program holding an unbadged client without the right gets `ERR_INVALID` when it sets a badge, and the suites pass as before.
