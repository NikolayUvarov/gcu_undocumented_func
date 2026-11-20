# Main task 352: installable media for PCs, ARM boards and phones

**From:** KRN (the kernel session, for UPD) · **To:** ALL · **Written:** 2026-11-20 19:32 UTC · **Concerns:** issues/352-installable-media-for-pcs-and-arm-devices.md on `main` (03c88c5144)

The maintainer asked for an ISO that installs MIND Core on a computer, and an image or package for ARM devices such as the Orange Pi boards and phones. Main task 352 (stage V, UPD) and its four tasks are on `main`, as plans (text only):

| Task | What | Another track's part |
|---|---|---|
| 352-UPD-0047 | A hybrid installer ISO for x86-64 and aarch64 UEFI; its live system's boot volume served from memory | KRN (the handover in `BootInfo`), done by this session |
| 352-UPD-0048 | The installer onto a disk of the same machine, or from a card onto the eMMC; the disk confirmed at the machine's console | PRT's disk tools (212), this session's |
| 352-UPD-0049 | Board and phone images from the one aarch64 release | MOB: a catalog of devices, in `issues/requests-MOB.md` |
| 352-UPD-0050 | Every artifact of a release from one commit, with a signed `SHA256SUMS` | — (publishing stays the maintainer's act; MIND Hub, 100003, is named as the server) |

Nothing here is a guarantee yet. Each task's criteria say which run, in QEMU or on which machine, would show it.

If your track has work that a release's media must carry, such as firmware that a driver reads, answer in KRN/. A proprietary file never goes into an image: it is copied onto the written disk.
