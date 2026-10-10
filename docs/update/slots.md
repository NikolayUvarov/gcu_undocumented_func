# Slots A and B: boot records, a trial and the fallback

**Version:** 0.6 (2026-10-10): the updater stages a slot and confirms a trial on the disk (351-UPD-0007); 0.5: a sequence number that cannot count down is refused (351-UPD-0015); 0.4 (2026-10-09): the updater's client holds the zone's badge (351-KRN-0022); 0.3: the update zone in `vfs_server` (351-UPD-0008) · **Track:** `UPD`, task [351-UPD-0006](../../issues-done/351-UPD-0006-slots-and-boot-records.done) · **Constitution:** MC-9.1, 9.3 · Russian: [slots_RU.md](slots_RU.md)

The bootloader can boot either of two copies of the system, slots A and B. Which one it boots is chosen by a boot record on the disk. A new slot runs on trial: it has a number of tries to be confirmed, and after them the bootloader goes back to the last confirmed slot. A slot that does not verify against its signed manifest is not booted. This gives an update its activation point, the record, and a configuration to return to (MC-9.3). The bootloader side is implemented and tested in QEMU. The running system knows its slot and whether it is on trial, and init confirms a healthy trial boot; the updater stages a release in the other slot, activates it with a trial record and writes the confirmed record ([updater.md](updater.md)).

## The layout

| Path | Holds |
|---|---|
| `MIND/A/`, `MIND/B/` | `kernel.elf`, the boot services and the build's `MANIFEST` and `MANIFEST.SIG`. The bootloader checks the slot's kernel and services against that manifest by name ([README.md](README.md)) |
| `MIND/BOOT0`, `MIND/BOOT1` | the two boot records, one sector each |
| `EFI/BOOT/` | the bootloader, outside the slots (351-UPD-0010 updates it) |
| the root | the applications, the licences and the voice model, shared by both slots |

A volume without `MIND/BOOT0` and `MIND/BOOT1` boots from its root, as before slots. The USB images and the volumes of most QEMU suites keep that layout until images are built with slots ([351-UPD-0016](../../issues/351-UPD-0016-images-with-slots.md)). `scripts/boot_slots.py layout VOLUME OUT [--both]` makes a slot volume from a build: its boot set moves into slot A (and is copied into B with `--both`), and slot A is confirmed.

## The boot record

512 bytes, little-endian. The same encoding is in `bootloader/src/slots.rs` and `scripts/boot_slots.py`.

| Offset | Size | Field |
|---|---|---|
| 0 | 8 | magic `MINDBOOT` |
| 8 | 4 | format, 1 |
| 12 | 8 | sequence number |
| 20 | 1 | slot: `A` or `B` |
| 21 | 1 | the slot to fall back to: `A`, `B` or 0 for none |
| 22 | 1 | tries left |
| 23 | 1 | flags: bit 0, confirmed |
| 24 | 484 | zeros |
| 508 | 4 | CRC-32 (IEEE, as zlib computes it) of bytes 0–507 |

A record of another size, magic or format, with a bad CRC, an unknown slot or flag, or anything but zeros in the reserved bytes is ignored. So is one whose sequence number is above 2^64 − 3: a trial writes one more, and its failure one more again, and each must count as newer (351-UPD-0015). A record file of zeros is empty: never written.

## What the bootloader does

1. **It reads both records** from its own volume and takes the valid one with the higher sequence number. It names a damaged record on the serial line (`BOOT: RECORD MIND\BOOT1 DAMAGED, IGNORED`).
2. **A confirmed record:** it boots that slot. If the slot does not verify, it boots the fallback slot.
3. **Not confirmed, with tries left:** before it loads anything it writes the same record with one try fewer and the next sequence number into the *other* record file. Then it boots the slot on trial (`BOOT: SLOT B LOADED ON TRIAL`).
   - If that write fails, the slot does not run on trial: a try that cannot be counted could repeat for ever. The fallback slot boots instead, or, with none, the slot itself (`BOOT: CANNOT RECORD THE TRY`). This case is not tested in QEMU yet.
   - If the slot does not verify, the bootloader writes the record once more with no tries left and boots the fallback slot.
4. **Not confirmed, no tries left:** the trial is over. It boots the fallback slot (`BOOT: SLOT B NOT CONFIRMED, NO TRIES LEFT`) and writes nothing.
5. **No valid record:** it tries slot A, then slot B.

A slot "verifies" as in [README.md](README.md): its manifest's signature, then the size and SHA-256 of each image. What it loaded is printed as the launch record, followed by `BOOT: SLOT <x> LOADED`. The console shows the slot too. If no slot verifies, the boot stops with `BOOT ERROR` and the last reason.

## Writing a record

Writing a record is the updater's job ([updater.md](updater.md), `updater/src/plan.rs`); `scripts/boot_slots.py` writes the same records into a disk image, for the tests and by hand.

- **Always write the other file:** the one that does not hold the newer valid record, with that record's sequence number plus one. A cut during the write leaves the newer valid record as it was, and the torn one fails its CRC. This covers the record. Damage to the FAT file system itself during the cut is examined in 351-ASR-0005.
- **Staging a slot:**
  - slot = the new one;
  - fallback = the slot now confirmed;
  - tries = N;
  - not confirmed.

  Command: `boot_slots.py stage IMAGE B --tries 3`.
- **Confirming:** the same slot and fallback, confirmed. Command: `boot_slots.py confirm IMAGE`. On the device, `init` confirms a healthy start to the kernel (351-KRN-0014), and the updater writes this record once the trial has outlived the kernel's deadline ([updater.md](updater.md)).
- `boot_slots.py show VOLUME|IMAGE` prints both records and which one counts.

## The update zone in the running system (351-UPD-0008)

`vfs_server` gives the updater's badge (`BADGE_UPDATE` in `libmind/src/fs.rs`) one zone on a volume booted from a slot, and nothing more:
- **the slot that did not boot** (`MIND/B` when A runs, `MIND/A` when B runs): files and directories inside it may be made, written, truncated, renamed within it and removed;
- **the two boot records** `MIND/BOOT0` and `MIND/BOOT1`: opened for writing only as they are (never made, emptied or removed), and written only whole: 512 bytes at offset 0 of a 512-byte record. The write changes the record's one sector in place, leaves its directory entry as it was, and is flushed before the call returns;
- everything else is read-only to it: the running slot, `EFI/`, the root's files, `data/`; private directories in `system/` stay hidden.

A system booted from the volume's root has no update zone. `vfs_server` logs which it is: `[VFS] UPDATE ZONE: MIND/B AND THE BOOT RECORDS, FOR THE UPDATER'S BADGE` or `[VFS] NO UPDATE ZONE: THE SYSTEM DID NOT BOOT FROM A SLOT`. Other badges, the shell's included, see the slots read-only as before.

`init` gives a client with that badge to the `updater` service alone (351-KRN-0022). Every other client of `vfs_server` it hands out carries a badge as well, readers `BADGE_READER` (`loader`'s, and so every program's, among them), and a badge is set once, so no holder can set the updater's on it.

## Tested

The tests run on x86 (`boot` suite) and aarch64 (`tests/aarch64_smoke.py`). Each uses a raw disk image with both slots and the build's bootloader, in QEMU (`tests/boot_slots_check.py`).
- Slot B staged with one try boots on trial, and the try is counted on the disk before it runs. Once its record is confirmed, B boots as confirmed.
- Staged again and not confirmed, B gives way to slot A at the next boot.
- Slot B with one byte of a service changed is not loaded. A boots and B is left with no tries.
- A newer record torn by a cut write is ignored for the older one.
- `fsck.fat` finds the file system consistent after the bootloader's writes.
- `vfs_server` names slot A as its update zone when B booted, and slot B when A did; on a root volume it has none.
- With the updater's test stand-in (`tests/updater_stub`) on a volume booted from slot A, its client badged by `init` writes a file in `MIND/B` and `MIND/BOOT0` whole with its own bytes, which the host then reads back; a part of a record, the record made afresh, a file in `MIND/A`, in `EFI/` and directly in `MIND/` are refused. `fsck.fat` finds the volume consistent afterwards.
- The volumes of these checks leave `keystore` out: it stores a new device key at the first boot, and a check that stops QEMU at a line could cut that write.
- Host tests (`tests/boot_slots_host.rs`) cover:
  - the encoding;
  - that every single flipped bit is refused;
  - the choice between two records.
- Host tests (`tests/vfs_zone_host.rs`) cover the update zone: what the updater's badge and every other badge may change below the boot root, and that a record write changes exactly one sector of the volume.
- The updater itself moves a slot volume from release 5 in slot A to release 6 in slot B: fetched over HTTPS, booted on trial through `init`'s restart and confirmed on the disk past the kernel's deadline; refused releases leave the records as they were (the `update` check, x86 and aarch64, [updater.md](updater.md)). Its records are host-tested against the bootloader's choice (`tests/updater_host.rs`).

The suites with a root volume show that a volume without records boots as before.

## Not provided yet

- **A confirmation the updater is told of.** Since 351-KRN-0014 the bootloader passes the slot, the trial flag, the manifest's digest and a 120 s deadline in `BootInfo`. init confirms a trial boot when every boot service started and the boot volume is mounted, and the kernel restarts an unconfirmed one at the deadline, so a hang or a boot without its volume uses up a try by itself. The updater learns of the confirmation only by outliving the deadline, so the record reaches the disk about two minutes after the boot ([updater.md](updater.md)).
- **Applications in the slots.** They stay at the root, shared, so a fallback runs the newer applications on the older kernel. Moving them needs the loader to know the booted slot (with 351-KRN-0014).
- **Rollback protection.** A fallback boots the other slot whatever its version (351-UPD-0009, 0011). The records are not signed: whoever can write the volume can choose a slot, but only one that verifies against a manifest signed with the boot key.
- **Images built with slots** ([351-UPD-0016](../../issues/351-UPD-0016-images-with-slots.md)).
- **A cut during the bootloader's own write, on real media** (351-ASR-0005).
