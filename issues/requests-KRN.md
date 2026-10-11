# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (4 requests waiting, 2026-10-11; the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI became [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash in the aarch64 boot suite is [000-KRN-0067](000-KRN-0067-the-aarch64-boot-test-on-pcu.md))

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

## The BCM4331's 802.11 core stopped before its bus mastering is turned off (550-KRN-0070, 550-DRV-0023)

**Recorded by:** the drivers track (`DRV`), 2026-10-11, from the MacBook Pro. The drivers track asked for 550-KRN-0070; this is its side effect.

### Problem

Since 550-KRN-0070, the kernel turns off the BCM4331's bus mastering at enumeration (`PCI: 04:00.0 CLASS 028000: BUS MASTERING TURNED OFF UNTIL A DRIVER HAS IT` in `hw0003.txt`), while the 802.11 core the firmware left running is still writing what it receives to memory. The core's agent then reports a pending backplane operation (reset status `00000002`) for the rest of the boot:

- every boot built with 550-KRN-0070 (from `fast-test` 221f5ab on) read `00000002` before `bcm_wifi` did anything; the two built without it (a00618b, b4e22f8) read `00000000`;
- an SMC reset does not change it: it is made anew at each boot;
- a reset pulse does not clear it; and while it is set, the first access to a register of the core stops the machine (550-DRV-0023, step 2, twice).

So the card cannot be brought up: Wi-Fi is blocked by this.

### Plan (a proposal; the kernel track decides)

Stop the core first, with bus mastering still on, the way Linux does on these Macs (its early quirk for the AirPort card), then turn bus mastering off as now. For `14E4:4331`, in `pci::enumerate` before `stop_mastering`:

- map BAR0 (16 KiB) for a moment; the core's agent is at BAR0+0x1000 (window 2, as the firmware leaves it; `bcm_wifi` logs `18101000`);
- wait up to 300 µs for the agent's reset status (+0x804) to read 0;
- write 1 to its reset control (+0x800), read it back, wait 1 µs, write 0, read it back, wait 10 µs;
- then clear bus mastering.

`bcm_wifi` keeps its own pulse; a core already idle takes a second one without harm.

### Acceptance criteria

On the MacBook Pro, `bcm_wifi`'s first line about the core reads reset status `00000000`, with bus mastering turned off at boot as now. The machine boots as before.

## A client of the updater for the shell

**Recorded by:** the update track (`UPD`), 2026-10-10, for [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done) and the tools track's `update` command (351-APP-0029, on its branch).

### Problem

`updater` serves `idl/update.wit` 1.0 (`check`, `fetch`, `apply`, `rollback`, `status`), but no task holds a client of it: it acts only through the automatic policy in `update.txt`. The shell's static slots 1 to 31 are all named in `common/abi.rs` (`SLOT_DYNAMIC` is 32), so a slot for the client is an ABI question.

### Plan (a proposal; the kernel track decides)

- A slot for the shell's client of `updater`, or another way for the shell to hold one.
- `init` gives the shell an unbadged client (the updater checks no badge: every request ends in a check of signed releases, and `apply` and `rollback` ask the user through the shell's command).
- The shell lends it to programs that ask for it, if the tools track wants `update` in `msh`.

### Acceptance criteria

The shell holds a client of `updater` (`caps` shows it), and `status` answers through it.

## init says whether it confirmed a trial boot

**Recorded by:** the update track (`UPD`), 2026-10-10, for [351-UPD-0007](../issues-done/351-UPD-0007-updater-service.done).

### Problem

`init` confirms a healthy trial boot to the kernel (`BOOT_CONFIRM`, 351-KRN-0014), and the confirmed boot record is the updater's to write. The updater cannot ask whether the boot was confirmed. It waits until 5 s past the kernel's deadline (120 s): had the boot not been confirmed, the kernel would have restarted the machine by then. That is sound but slow. The other slot stays untouched for two minutes after every update, and the QEMU `update` check waits that long.

### Plan (a proposal; the kernel track decides)

- `init.wit` 1.4: `boot: func() -> result<boot, error>`, with the slot, whether it booted on trial and whether `init` confirmed it. Any client of `init` may call it, or only the `BADGE_REBOOT` one.
- The updater asks for it every second while it waits, and keeps the deadline as the bound.

### Acceptance criteria

On a trial boot, the updater writes the confirmed record within a few seconds of `[INIT] TRIAL BOOT CONFIRMED`, and the `update` check passes.
