# Slots A and B: boot records, a trial and the fallback

**Version:** 0.9 (2026-10-25): the version floor (351-UPD-0024); 0.8 (2026-10-24): each slot carries its own loader, which the loader at `EFI/BOOT/` checks and starts (351-UPD-0010); 0.7 (2026-10-22): the applications in the slot with the services (351-UPD-0023); 0.6 (2026-10-10): the updater stages a slot and confirms a trial on the disk (351-UPD-0007); 0.5: a sequence number that cannot count down is refused (351-UPD-0015); 0.4 (2026-10-09): the updater's client holds the zone's badge (351-KRN-0022); 0.3: the update zone in `vfs_server` (351-UPD-0008) · **Track:** `UPD`, task [351-UPD-0006](../../issues-done/351-UPD-0006-slots-and-boot-records.done) · **Constitution:** MC-9.1, 9.3 · Russian: [slots_RU.md](slots_RU.md)

The bootloader can boot either of two copies of the system, slots A and B. Which one it boots is chosen by a boot record on the disk. A new slot runs on trial: it has a number of tries to be confirmed, and after them the bootloader goes back to the last confirmed slot. A slot that does not verify against its signed manifest is not booted. This gives an update its activation point, the record, and a configuration to return to (MC-9.3). The bootloader side is implemented and tested in QEMU. The running system knows its slot and whether it is on trial, and init confirms a healthy trial boot; the updater stages a release in the other slot, activates it with a trial record and writes the confirmed record ([updater.md](updater.md)).

## The layout

| Path | Holds |
|---|---|
| `MIND/A/`, `MIND/B/` | `kernel.elf`, the boot services, the applications, the slot's own loader `BOOTX64.EFI` (`BOOTAA64.EFI` on aarch64, 351-UPD-0010) and the build's `MANIFEST` and `MANIFEST.SIG` (351-UPD-0023). The loader opens an application from the booted slot when the slot holds any (351-KRN-0151); a slot staged before holds none, and the root's run. The bootloader checks the slot's kernel and services against that manifest by name ([README.md](README.md)) |
| `MIND/BOOT0`, `MIND/BOOT1` | the two boot records, one sector each |
| `MIND/FLOOR` | the version floor, one sector: no slot whose manifest names a lower version boots ([below](#the-version-floor-351-upd-0024)) |
| `EFI/BOOT/` | the loader the firmware starts, outside the slots. An update never rewrites it: it starts a slot's own loader ([below](#a-slots-own-loader-351-upd-0010)) |
| the root | the licences and the voice model, shared by both slots |

A volume without `MIND/BOOT0` and `MIND/BOOT1` boots from its root, as before slots. The volumes of most QEMU suites keep that layout, and so does a USB image unless it is made with `make_usb_image.py --slots` ([write-disk.md](../write-disk.md), 351-UPD-0016). `scripts/boot_slots.py layout VOLUME OUT [--both]` makes a slot volume from a build: its boot set moves into slot A (and is copied into B with `--both`), the loader is copied there from `EFI/BOOT/`, and slot A is confirmed.

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

## A slot's own loader (351-UPD-0010)

The loader at `EFI/BOOT/` is the file the firmware starts, and the one `bless` names on a Mac ([write-disk.md](../write-disk.md), section 6). An update never rewrites it. Each slot carries its own copy of the loader instead, `MIND/<slot>/BOOTX64.EFI`, listed in the slot's signed manifest as `EFI/BOOT/BOOTX64.EFI`, so a release brings its own loader.

1. **The loader at `EFI/BOOT/`** chooses the slot and counts a trial's try as above, before anything else.
2. **A slot with no loader of its own** (one staged before this), **or with the same file:** the loader loads the slot itself, as above. "The same" means its SHA-256 is that of the file the firmware started, read by the path the firmware names (else `EFI/BOOT/`'s).
3. **Another loader** is checked against the slot's manifest: the signature first, then the loader's size and SHA-256. One that is not as listed is never started, and the slot counts as one that does not verify (`BOOT: SLOT B: EFI/BOOT/BOOTX64.EFI: not as the manifest says`).
4. **It is started from the bytes checked:** `LoadImage` from a buffer, with the file's device path so that it reads its own volume (`BOOT: SLOT B: STARTING ITS OWN LOADER, SHA-256 …`). Its load options tell it the slot, the trial and a safe start: `MIND-CHAIN 1 B 1 0` in UCS-2.
5. **The slot's loader** says so (`BOOT: SLOT B'S OWN LOADER, STARTED BY THE LOADER AT EFI/BOOT/, ON TRIAL`).
   - It reads no record and writes none. It asks for neither the first pause nor the safe start's key: the first loader did.
   - It loads its slot against the slot's manifest, as above.
6. **If it cannot,** it says why (`…; BACK TO THE LOADER AT EFI/BOOT/`), gives its buffers back and returns.
   - The loader at `EFI/BOOT/` then treats the slot as one that does not verify. A trial's record is left with no tries, and the fallback slot boots at once.
   - A loader the firmware refuses to load (Secure Boot, [secure-boot.md](secure-boot.md)) is the same case.
7. **A slot's loader that hangs, or a reset while it runs:** the try was counted before it started, so the next boot past the trial's tries is the fallback's.

Load options of any other form, or none, mean the firmware started the loader: it chooses the slot itself. The options choose only among the slots of its own volume, each checked against its signed manifest.

A slot's kernel thus always gets the handoff of its own build's loader. `EFI/BOOT/BOOTX64.EFI` itself changes only with a new disk, and a loader there from before this task boots every slot itself.

**Who puts the loader in a slot:**
- `make_usb_image.py --slots` and `boot_slots.py layout` copy the build's loader into slot A (and B with `--both`): the same file as at `EFI/BOOT/`, so a new disk starts no second loader;
- the updater and `scripts/link_update.py` stage the release's loader into the slot with its kernel and services ([updater.md](updater.md)).

## The version floor (351-UPD-0024)

A signature says a release is ours, not that it is current: an old release with a known hole stays validly signed. So a record written by whoever can write the volume could boot it. The floor stops that, and still lets a release with a bug be left (MC-9.4: an unauthorized rollback to a vulnerable configuration is refused, a permitted one is not).

- **The version is under the boot key:** a manifest's `version N` header line (`sign_manifest.py --version N`; `release.py stage` refuses a manifest whose version is not the release's). A manifest without one counts as version 0.
- **`MIND/FLOOR`** holds the floor: magic `MINDFLOR`, format 1, the floor (u64), zeros, and a CRC-32 of the first 508 bytes, as a record. It lies beside the records, not in them, so loaders from before this read the records as before. An image with slots starts at 0 (`boot_slots.py layout`, `make_usb_image.py --slots`); `boot_slots.py floor IMAGE [N]` shows or sets it.
- **The bootloader** reads it and refuses a slot whose manifest names a lower version, before it loads or starts anything of it (`BOOT: MIND\B\: VERSION 5 BELOW THE FLOOR 6`). The slot then counts as one that does not verify: a trial's tries are spent and the fallback boots. A damaged floor is said and counts as 0.
- **The floor is the channel's minimum,** not the highest version confirmed here. The publisher raises a channel's minimum only for a fix (`release.py publish --minimum N`); without it the minimum stays as published. An ordinary release can therefore still be rolled back from, to any release at or above the minimum.
- **The updater raises it,** in place and never down, to the highest minimum it has seen in a checked channel, once the running slot is confirmed and its own manifest names that version or above. Lowering it is the recovery authority's (351-UPD-0030).
- **The limit:** the floor is a file on the same disk. Whoever has the disk in hand can lower it; the floor that only goes up is a TPM counter (351-UPD-0011).

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
- A USB image made with `make_usb_image.py --slots --channel` boots from slot A, `vfs_server` names slot B as the update zone, and the updater reads the running version from the channel kept in slot A (`tests/usb_image_smoke.py`, x86).
- **A slot's own loader** (351-UPD-0010; `own_loaders`, x86 and aarch64). Slot B holds the build's loader with bytes past its sections: another file, which loads the same, listed in B's manifest.
  - B boots through it on trial. Its try is counted once, by the loader at `EFI/BOOT/`, before the slot's loader starts. Once confirmed, B boots through it again.
  - A reset while it waits (its pause for a photo) leaves the trial spent, and the next boot is slot A's.
  - With a service of B damaged it returns. Slot A boots at once, and B is left with no tries.
  - One that is not as B's manifest lists it is never started, and slot A boots.
  - A slot without a loader of its own boots through the loader at `EFI/BOOT/`, as before.
  - In the other checks both slots hold the build's loader, the same file as `EFI/BOOT/`'s, and no second loader is started.
- **The version floor** (351-UPD-0024; `version_floor`, x86 and aarch64): under floor 6, slot B's release 5 is not booted and slot A boots, B left with no tries; at floor 5 it boots on trial. In the `update` check release 6 raises the minimum to 6: once it is confirmed the floor is 6, and a record that boots slot A's release 5 boots slot B. Host tests: the floor's encoding and every flipped bit (`tests/boot_slots_host.rs`), and that the updater only raises it, to a minimum the confirmed slot meets (`tests/updater_host.rs`).
- The updater itself moves a slot volume from release 5 in slot A to release 6 in slot B: fetched over HTTPS, booted on trial through `init`'s restart and confirmed on the disk past the kernel's deadline; refused releases leave the records as they were (the `update` check, x86 and aarch64, [updater.md](updater.md)). Release 6 there, and in the `linkupdate` check, carries a loader of its own (bytes past its sections): the updater stages it into slot B, and the trial boots through it. Its records are host-tested against the bootloader's choice (`tests/updater_host.rs`).

The suites with a root volume show that a volume without records boots as before.

## Not provided yet

- **A slot's own loader on hardware.** It is tried in QEMU only (OVMF, AAVMF). A machine starts slots' loaders once its disk is written with a loader from this task at `EFI/BOOT/`: the MacBook Pro's stick is written once more for that. Apple's firmware's `LoadImage` from a buffer is not tried.
- **A loader that differs in what it does.** The checks start a loader whose bytes differ, not its code.

- **A confirmation the updater is told of.** Since 351-KRN-0014 the bootloader passes the slot, the trial flag, the manifest's digest and a 120 s deadline in `BootInfo`. init confirms a trial boot when every boot service started and the boot volume is mounted, and the kernel restarts an unconfirmed one at the deadline, so a hang or a boot without its volume uses up a try by itself. The updater learns of the confirmation only by outliving the deadline, so the record reaches the disk about two minutes after the boot ([updater.md](updater.md)).
- **A floor someone with the disk cannot lower** (351-UPD-0011, a TPM counter). The version floor above stops a record written over the network or by a program; the records and the floor are not signed, so whoever can write the volume can still choose a slot, and lower the floor, but only among releases signed with the boot key.
- **Slot images by default.** `make_usb_image.py` lays the build out in slots only with `--slots` ([write-disk.md](../write-disk.md)); `files` still reads the kernel at the root ([requests-APP.md](../../issues/requests-APP.md)).
- **A cut during the bootloader's own write, on real media** (351-ASR-0005).
