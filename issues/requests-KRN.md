# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (7 requests waiting, 2026-10-10; the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md), `bcm_wifi` as a boot service [550-KRN-0059](../issues-done/550-KRN-0059-bcm-wifi-at-boot.done), and the tools track's `SLOT_SHELL` and `SLOT_CLIPBOARD` from its branch [211-KRN-0058](211-KRN-0058-slots-for-the-shell-and-the-clipboard.md); the updater's badged VFS client is done in [351-KRN-0022](../issues-done/351-KRN-0022-updater-grants.done), a flush's stale failure (also 211-DRV-0019's request) in [211-KRN-0067](../issues-done/211-KRN-0067-a-flush-after-a-failed-one.done)) · **Recorded by:** the tools track (APP), 2026-10-06

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

## Bus mastering off at boot until a driver is granted the device (550-DRV-0022)

**Recorded by:** the drivers track (`DRV`), 2026-10-10, from the MacBook Pro's run of [550-DRV-0020](../issues-done/550-DRV-0020-bcm4331-read-only-probe.done).

### Problem

The kernel turns bus mastering on when it grants a device's resource to a driver (`pci::enable`) and off before a driver is restarted (`pci::quiesce`). It leaves every other function's command register as the firmware set it. On the MacBook Pro the firmware leaves the BCM4331 with bus mastering on and its 802.11 core running (command `0006` in `hw0001.txt`). `bcm_wifi` now holds that core in reset when it starts ([550-DRV-0022](550-DRV-0022-bcm4331-core-reset-and-sprom.md)), about 5 s into the boot. Until then, and for any device no driver is given, a device the firmware left running may write to memory. The profile declares no IOMMU (MC-1.5), so nothing else stops it.

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

## QEMU's vvfat crashes in the aarch64 boot suite on `main` at 661147f

**Recorded by:** the storage session, 2026-10-09, from its local gate and runs of `tests/aarch64_smoke.py` on plain `main`.

### Problem

The group "aarch64: boot and fault containment" (`tests/aarch64_smoke.py`) fails because QEMU itself stops:

```
qemu-system-aarch64: block/vvfat.c:2760: handle_renames_and_mkdirs: Assertion `j < s->mapping.next' failed.
```

The test boots from a directory served as `fat:rw:` (vvfat), and the crash comes after `[INIT] READY`. That is when services write to the boot volume (`keystore` makes `system/keystore` and its key on a fresh disk), and in the fault cases while `rtc` restarts.

Measured on this machine, with the build of each tree:
- `main` at 661147f: fails 2 runs in 3.
- `main` at 661147f with `vfs_server/src/fat.rs` as it was before b8b172f (175-KRN-0047…0049): fails 2 runs in 4. So it is not the FAT audit fix.
- The same group passed in the storage branch's gate merged with `main` at a9ac93f, before the kernel branch's 32 commits came in. That was one run, so it does not prove the group was reliable before.

### Plan (a proposal; the kernel track decides)

- Find which of the commits between a9ac93f and 661147f changes the writes vvfat sees, or the timing.
- Either way, QEMU documents vvfat with `rw` as unreliable. The boot suite could boot from a raw FAT image made with mtools, as `tests/boot_slots_check.py` does, and keep vvfat for read-only directories.

### Acceptance criteria

The group passes in repeated runs (say 5 of 5) on `main`.

## Free clusters counted a FAT sector at a time

**Recorded by:** the storage session, 2026-10-10, from its local gate (the branch at 68d3b80 merged with `main` at 8f05728), for the `disks` check of 251-KRN-0031 and [251-STO-0014](../issues-done/251-STO-0014-importing-a-model-disk.done).

### Problem

`Volume::free_clusters` (`vfs_server/src/fat.rs`) calls `fat(cluster)` once per cluster, and each call reads its FAT sector through the cache: a zeroed 512-byte buffer, a search of 64 tags and a copy. A model disk's FAT32 volume of 256 MiB has 65 527 clusters, so the first `df` after it is mounted does that 65 527 times.

Under TCG on aarch64 this took 2.2 s with `main`'s `vfs_server` and 9.0 to 9.6 s with the branch's, whose only difference in the call path is that a function was added elsewhere (`Volume::overwrite`). Every function on the path has the same size in both builds, so the time follows where the code lands. Over 8 s the `disks` check's `df` timed out in two groups ("aarch64: files, network and TLS", "aarch64: NVMe boot disk"). The second `df`, from the counted value, took 0.3 s. The branch gives that `df` 60 s for now.

### Plan (a proposal; the kernel track decides)

Count the free entries of each FAT sector read once: 512 sectors for that volume instead of 65 527 calls. A host test in `tests/fat_host.rs` can compare the count with the per-cluster one on FAT12, 16 and 32.

### Acceptance criteria

The first `df` of a 256 MiB FAT32 volume takes well under a second on aarch64 under TCG, and the counts match the per-cluster ones in the host test.

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

