# 351-UPD-0024 — A version floor in the boot records, kept by the bootloader

**Type:** update (boot) · **Owner:** `UPD` track, with `KRN`/`PRT` for `bootloader/` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-12.4

Split from [351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md) (the maintainer, 2026-10-23). The floor is the channel's minimum alone, not the highest version confirmed here: the maintainer, 2026-10-25, so that a release with a bug can still be rolled back.

## Problem

The updater refuses a rollback below the highest minimum it has seen (`rollback`, docs/update/updater.md). The bootloader does not: it boots either slot whatever its version. So a fallback, or a record written by whoever can write the volume, boots an older signed release (docs/update/slots.md, "Rollback protection"). Nothing at boot knows a slot's version: the manifest's header lines are read by neither the bootloader nor the updater.

## Plan

- **The version under the boot key.** The manifest gets a `version N` header line, which format 1 already allows. `scripts/sign_manifest.py` and `scripts/release.py` write it, so the boot key vouches for the version, not the channel alone. A manifest without it counts as version 0.
- **Boot records, format 2.** They add the floor: the highest minimum the updater has seen in a channel checked with the release key. Format 1 records are not read: the bootloader and the tools refuse them with a message naming the format, and a disk that has them is written again by hand (the maintainer, 2026-10-25: no compatibility with our own older formats before the first release). The refusal is the explicit transition (MC-12.4).
- **The floor is not the highest version confirmed here.** The publisher raises a channel's minimum only when a release fixes a hole, so that the older ones count as vulnerable; an ordinary release leaves it as it was. A rollback to an earlier release at or above the minimum stays the owner's choice: MC-9.4 forbids an unauthorized rollback to a vulnerable configuration, not every rollback.
- **The bootloader refuses a slot below the floor.** It boots the other slot if that one verifies and is not below. Otherwise it stops with `BOOT ERROR: MANIFEST: below the version floor`.
- **The updater only raises the floor,** to a higher minimum a checked channel names, once the running slot is confirmed at or above it. Confirming a trial raises nothing.
  - A slot left below the new floor is no fallback any more. The updater logs it, and the next update writes over that slot.
  - Lowering the floor is the recovery authority's ([351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md)).
- **A release with a bug can still be left:**
  - a trial that is not confirmed falls back as today;
  - the updater's `rollback` boots the other slot while it is at or above the floor;
  - below the floor, a fixed newer release, or the recovery authority.
- `scripts/boot_slots.py` reads and writes format 2; the host tests are in `tests/boot_slots_host.rs`.
- **The limit, stated plainly** in docs/update/slots.md and the profile: the records are not signed, so someone with the disk in hand can lower the floor. The floor that only goes up is [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md), with [351-UPD-0012](351-UPD-0012-secure-boot-with-our-own-keys.md).
- The bootloader change is made together with `KRN`/`PRT` (TRACKS.md).

## Acceptance criteria

- Host tests:
  - a record pair whose floor is above one slot's version chooses the other slot;
  - a format 1 record is refused, with a message naming its format;
  - no write of the updater lowers the floor;
  - confirming a trial leaves the floor where it was.
- QEMU, x86 and aarch64 (a rollback still allowed): with the floor at 5, release 7 confirmed in slot B, a rollback boots release 6 in slot A.
- QEMU, x86 and aarch64 (a replayed older release):
  - a record written by the host that boots slot B with release 5, under floor 6, is refused, and slot A with release 6 boots;
  - the serial line names the refusal.
- The `update` suite: after release 7 is confirmed and the channel's minimum is 7, a fallback to release 6 is not booted.
- docs/update/slots.md and slots_RU.md give format 2 and the limit.

## Related

[351-UPD-0009](351-UPD-0009-rollback-policy-and-key-roles.md), [351-UPD-0011](351-UPD-0011-version-floor-in-the-tpm.md), [351-UPD-0006](../issues-done/351-UPD-0006-slots-and-boot-records.done), [351-UPD-0015](../issues-done/351-UPD-0015-a-sequence-that-cannot-count-down.done), [351-UPD-0030](351-UPD-0030-a-separate-recovery-authority.md).

## Progress (2026-10-25)

**Landed in round 34** (`main` 1c90f553), as a first form:
- **The version.** A manifest's `version N` line under the boot key; `release.py` carries the channel's minimum over and `stage` checks the version; `link_update.py` signs with the release's version.
- **The bootloader** refuses a slot below the floor, with the reason on the serial line, and boots the other one.
- **The updater** raises the floor only to a checked channel's minimum, once the running slot is confirmed at or above it.
- **Where the floor is kept:** in a file of its own, `MIND/FLOOR` (512 bytes with a CRC-32), with the records left in format 1.
- **Tests:**
  - `version_floor` in the x86 `boot` suite and `tests/aarch64_smoke.py`;
  - the `update` and `linkupdate` suites on both architectures, with releases 5 and 6 rather than 6 and 7;
  - the host tests.

**Left.** A file of its own was chosen so that the loader already on the MacBook Pro's stick could still read the records: it reads only format 1. That compatibility is not wanted before the first release: a disk is written again by hand (the maintainer, 2026-10-25; AGENTS.md, section 3). So the plan's format 2 stands:
- **The floor moves into the boot records.** Both records of the pair carry it, so a torn write of one leaves the floor in the other; a damaged `MIND/FLOOR` now counts as 0.
- **Format 1 records are refused, not read.** `MIND/FLOOR` and its code go.
- **The MacBook Pro's stick** is written again by hand.
