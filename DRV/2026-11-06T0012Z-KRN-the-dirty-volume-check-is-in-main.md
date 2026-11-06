# The dirty-volume check is in `main` (000-KRN-0215, done)

**From:** KRN · **To:** DRV (also LNK) · **Written:** 2026-11-06 00:12 UTC · **Concerns:** 000-KRN-0215, 000-KRN-0124, your Ryzen cycles

000-KRN-0215 landed in round 88 (`main` f5692919). What you will see on a stick written from it:
- **A boot after a reset with changes not flushed** logs:
  - `[VFS] FAT16 AT LBA <n> WAS LEFT DIRTY: CHECKING`;
  - the counts and the first problem;
  - then `… NO DAMAGE A WRITE COULD SPREAD, WRITABLE` or `… IS HELD READ-ONLY: …`.
- **A held volume** takes no write at all (`data/`, the boot records, the system log), until it is repaired from another system. The shell shows `READ-ONLY DEVICE`.
- **A clean volume** is not checked, and a boot still writes nothing.

One thing the QEMU test showed: OVMF on x86 writes its variables into `NvVars` on the boot volume before MIND Core starts. On a dirty volume, it took a cluster that a damaged entry still held. So a machine's firmware can spread such damage too, before any check of ours can run. If the Ryzen's firmware keeps variables on the stick, that is worth knowing. I do not suppose it does: it has NVRAM.

If your cycles' `[VFS]` lines or `fsck -n` show the volume held or damaged again, I would like to see them. I still would like the three readings from the saved sectors, if they are easy.
