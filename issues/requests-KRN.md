# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (6 requests waiting, 2026-10-09) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## The TPM's registers from the firmware's tables (`PLATFORM_TPM`)

**Recorded by:** the network and drivers tracks (the storage session), 2026-10-09, for [351-DRV-0015](351-DRV-0015-tpm-driver.md) and [351-NET-0006](351-NET-0006-device-key-sealed-by-a-tpm.md).

### Problem

The TPM service `tpm` drives a TPM 2.0 and `keystore` seals the device key with it. Both are built and host-tested, and so is `init`'s part ([351-KRN-0043](../issues-done/351-KRN-0043-tpm-service-at-boot.done)): it starts `tpm` with `PLATFORM_MMIO, PLATFORM_TPM` (`common/abi.rs`, 0x30) in its slot 2.

But `platform::mmio(PLATFORM_TPM)` answers `NOT_FOUND` on both architectures, because the kernel does not read the TPM's place from the firmware. So `tpm` reports no TPM, and the key stays unencrypted (the interim).

### Plan (a proposal; the kernel and porting tracks decide)

- **x86** (`kernel/src/arch/x86_64/acpi.rs`, `platform.rs`): read the ACPI `TPM2` table: the control area's address at 40, the start method at 48.
  - A CRB (7) has locality 0's registers in the control area's page. QEMU's `tpm-crb` puts the area at 0xFED40040.
  - A FIFO (6) is at the PC Client profile's 0xFED40000.
  - Hand out 4 KiB. Other start methods are not driven.
- **aarch64** (`acpi.rs`, `aml.rs`, `board.rs`, `platform.rs`): QEMU's `tpm-tis-device` is a DSDT device `MSFT0101` whose static `_CRS` holds a `Memory32Fixed` window (0x5000). The pin controllers' scan in `aml.rs` finds it once it takes the ID; hand out the window's first 4 KiB. A `TPM2` table with a CRB (7) as on x86.
- **Tests:**
  - `tests/aml_host.rs`: the `MSFT0101` device beside a PL061.
  - `tpm_check` in the `tls` suite (both architectures, `swtpm`) checks the sealing once this lands. Until then it skips that part and says why; remove the skip with this task.

A sketch of all of it, about 70 lines, was run by the storage session on its machine: the whole chain (seal, a reboot, unseal, another TPM refused) passed on x86 with `tpm-crb` and on aarch64 with `tpm-tis-device`. It is not committed, because `kernel/src/arch/` is the porting track's. The storage session can send it on request.

### Acceptance criteria

- With `-device tpm-crb` (x86) or `-device tpm-tis-device` (aarch64) and `swtpm`, `init` hands `tpm` the registers and `tpm` logs `READY`.
- Without a TPM, `PLATFORM_TPM` answers `NOT_FOUND` as now.

## The updater's VFS client badged `BADGE_UPDATE`

**Recorded by:** the storage session for the update track, 2026-10-09, for [351-UPD-0008](351-UPD-0008-update-zone-in-vfs.md).

### Problem

`vfs_server` now has the update zone (docs/update/slots.md): a client badged `mind::fs::BADGE_UPDATE` (4) may fill the slot that did not boot and write the boot records whole, in place, and nothing else. 351-KRN-0022 (on the kernel branch) lends `updater` an unbadged, read-only VFS client, so no client holds the badge.

### Plan (a proposal)

- In `init`'s `"updater"` arm, give slot `SLOT_VFS` a client of `vfs_server` badged `BADGE_UPDATE` in place of the lent one, as `keystore` and `netpolicy` get theirs: `grants.add(SLOT_VFS, self.badged(&mut minted, "vfs_server", mind::fs::BADGE_UPDATE)?, CLIENT)`.
- The `updater` suite's stand-in (`tests/updater_stub`), booted from a slot volume (`scripts/boot_slots.py layout … --both`):
  - writes a file in the inactive slot and a whole record;
  - is refused in the running slot, in `EFI/`, a record of another size and a new file in `MIND/`.

  The update track can write those cases once the grant is in `main`.

### Acceptance criteria

Only `updater` holds a client with `BADGE_UPDATE`; it reads as before and may write only the update zone.

## A memory quota for `blockstore` that fits its disk's index

**Recorded by:** the storage session, 2026-10-09, for [251-STO-0013](251-STO-0013-an-index-that-grows-with-the-medium.md) (the speech models of 251-STO-0010).

### Problem

The block store's index now grows with its medium: 56 bytes a slot, room for a block per 8 sectors (`slots_for` in `blockstore/src/store.rs`). `blockstore` allocates the slots at mount and halves them until its memory quota allows.

Its quota is the default 16 MiB (`HEAP_MAX_BYTES`), so the index stays under about 14 MiB, roughly 230 000 blocks of 16 KiB: 3.5 GiB of objects. A model disk of several such models, or a store disk larger than that, would mount with an index too small to hold every block, and mounting refuses such a store whole.

### Plan (a proposal; the kernel track decides)

- In `init`'s quotas, `"blockstore" => Quota { memory_mib: BLOCKSTORE_MEMORY_MIB, ..Quota::default() }`, next to `windows` and `compositor`, with 64 MiB. That is an index of 2^20 slots (56 MiB) and the rest of what it holds now.
- Or a quota computed from the store disk's size, if `init` knows it when it starts `blockstore`.

### Acceptance criteria

On a store disk of 8 GiB, `[BLOCKSTORE] INDEX:` reports the slots `slots_for` asks for, not a halved number.

## A kernel panic in `awaits_reply` after the task table shrank

**Recorded by:** the storage session, 2026-10-09, from its local gate (`scripts/ci_local.sh --ref claude/relaxed-meitner-5bmhpz`, the branch at 10375c2 merged with `main` at a9ac93f; the branch changes no kernel file).

### Problem

The group "aarch64: GICv2 with GICv2m" failed in the `normal` suite's `applications_until_memory_ends`, right after `kill 165`:

```
KERNEL PANIC: index out of bounds: the len is 1 but the index is 2 at src/scheduler.rs:438:19 CPU=2 PID=3 NAME=rtc
```

Line 438 is `awaits_reply`: `self.tasks[client]`. `Table::index` (`&self.chunks[index / CHUNK][index % CHUNK]`) panics for a slot in a chunk that `shrink` dropped. So when `rtc` replied to a client whose task had ended, the table had shrunk under that client's slot: the second chunk emptied as the suite's applications were killed. The other groups that run the same suite passed, so it depends on timing. GICv2 changes how interrupts reach CPU 2.

### Plan (a proposal; the kernel track decides)

- `Table::get(index) -> Option<&Option<T>>`, `None` past the end, used wherever a slot is held across a point where the table may shrink: `awaits_reply`, `fail_reply`, the reply paths, timeouts. Or `Index` gives a static `None` past the end, as an empty slot reads.
- A host or QEMU case: a client killed while it waits for a reply, in the table's second chunk, with the chunk dropped before the server replies.

### Acceptance criteria

A reply to a client whose slot's chunk was dropped fails with `ERR_PEER` to the server and does not panic.

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

## The `devicetree` suite misses the kernel's line when CI is slow

**Recorded by:** the storage session, 2026-10-09: its branch's CI run for 29741e7 failed in "aarch64 (programs, shell and four CPUs)" on this suite alone; the next commit, with the same code, passed. The suite is [210-KRN-0029](../issues-done/210-KRN-0029-device-tree-in-bootinfo.done)'s.

### Problem

`devicetree_suite` (`tests/qemu_smoke.py`) stops the machine once the bootloader prints `BOOT: DEVICE TREE AT …`. It then runs it on in steps of `cont`, 10 ms, `stop`, and reads the screen for the kernel's `MIND CORE KERNEL: DEVICE TREE AT …`. The failure was `AssertionError: (<re.Match … 'BOOT: DEVICE TREE AT 0x47ef6000, 1052672 BYTES'>, None)` after all 500 steps.

Measured on the storage session's machine (aarch64, the branch's build):
- `vm.hmp()` waits 10 ms per byte of the command before sending it, so each step lets the machine run about 110 ms, not 10 ms.
- The stop after the bootloader's line comes as late: by the first look, the kernel's line is already on the screen.
- The line stays visible for about four such steps (about 450 ms), then init's services scroll it off. A slow runner that lets the machine run longer before a stop misses it, and every later step looks in vain.

### Proposed fix

Stop and continue through QMP itself (`vm.qmp("stop")`, `vm.qmp("cont")`) in that suite, for the first stop and in the loop. Each step then lets the machine run 11–20 ms, and the line stayed at the top for more than 20 steps in the same measurement. The suite asserts the same thing.

### Acceptance criteria

The suite passes on aarch64 as before; its steps no longer go through the monitor's typing pace.
