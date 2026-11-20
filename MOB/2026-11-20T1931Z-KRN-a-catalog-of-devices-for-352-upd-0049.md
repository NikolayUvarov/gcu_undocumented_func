# A catalog of devices for board_image.py, for 352-UPD-0049

**From:** KRN (the kernel session, for UPD) · **To:** MOB · **Written:** 2026-11-20 19:31 UTC · **Concerns:** [issues/requests-MOB.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-MOB.md), 352-UPD-0049, 100004-MOB-0001

The maintainer asked today for an installable ISO for computers, and an image or package for ARM devices such as the Orange Pi boards and phones. None existed, so main task 352 and its tasks are on `main` (03c88c5144, text only):
- 352-UPD-0047: an installer ISO, x86-64 and aarch64 UEFI;
- 352-UPD-0048: the installer onto a disk of the same machine, and from a card onto the eMMC;
- 352-UPD-0049: board and phone images from the one aarch64 release;
- 352-UPD-0050: every artifact of a release from one commit, with a signed SHA256SUMS.

**Your part, recorded in `issues/requests-MOB.md` (request 1).** Your scripts make the boards' cards (100004-MOB-0001), so 352-UPD-0049 builds on them and does not change them. It asks for:
1. one catalog entry per board or phone, read by `board_image.py --board <name>`:
   - the SoC;
   - the firmware's script, source, version and SHA-256;
   - where the firmware goes, for a card boot and for an eMMC boot;
   - the boot partition's start and the device tree;
2. `board_image.py` taking a boot volume given to it (a release's FAT image), not only a local build. UPD adds the manifest check;
3. the two Orange Pi boards first, the PinePhone when it is at hand.

Nothing in this asks you to change U-Boot's builds or the boards' runs (100004-MOB-0002, 0003). Number it as you see fit, or answer here if the catalog should look different.
