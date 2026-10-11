# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (1 request waiting, 2026-10-11; the BCM4331's core stopped before its bus mastering became [550-KRN-0082](550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.md), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](000-KRN-0067-the-aarch64-boot-test-on-pcu.md))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## A drive that comes back is checked to hold the same volumes before anything is written (211-DRV-0019)

**Recorded by:** the drivers track (`DRV`), 2026-10-11, from the MacBook Pro.

### Problem

The maintainer unplugged the boot disk from a running MacBook Pro (717 s into the boot), wrote a new image onto it on another computer, and plugged it into the same running system again (1109 s). `usb_storage` logged `THE DEVICE IS BACK, THE SAME CAPACITY` and `vfs_server` `THE USB DRIVE ANSWERS AGAIN`. The old system then wrote its journal into the new image's log volume (`LOG:boot0001.log`, 681 bytes, on a disk whose log volume the image had just made empty).

This time the new boot volume's 98 files still match the build, and the next boot ran. But `vfs_server` kept the old volumes' FAT state and up to 64 cached sectors, so it can write the old state over a different file system.

`usb_storage` cannot tell such a disk from the one it had: our images have the same capacity, the same MBR disk ID (`BE1AFDFA`) and the same partitions, and their FAT serials are fixed. What differs is the volumes' contents, which `vfs_server` knows (211-KRN-0012: the boot volume is identified by the manifest the bootloader verified).

### Plan (a proposal; the kernel track decides)

- When a drive answers again after it did not, `vfs_server` reads each mounted volume's boot sector and its first FAT sector again, before any write. For the boot volume it also checks the manifest.
- If they differ from what it holds, it drops that volume's cached sectors and marks the volume gone (`ERR_IO`, the log says so). The new medium is mounted as new, or not at all until the next boot, as the track prefers.
- `usb_storage` (`DRV`) keeps reporting what it sees: `THE DEVICE IS BACK, THE SAME CAPACITY` stays true and says nothing about the contents.

### Acceptance criteria

In QEMU, `device_del` of the USB boot disk, its image replaced by another build's, then `device_add`: nothing from the old system's cache is written to the new image (`fsck.fat` clean, the new files unchanged), and the log says the volume changed. The same disk plugged in again unchanged still works as now (211-KRN-0050's test).
