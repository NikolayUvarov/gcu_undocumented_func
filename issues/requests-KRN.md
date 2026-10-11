# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (6 requests waiting, 2026-10-11; the BCM4331's core stopped before its bus mastering became [550-KRN-0082](550-KRN-0082-the-bcm4331-core-stopped-before-its-bus-mastering.md), the badge right became [000-KRN-0073](../issues-done/000-KRN-0073-a-badge-set-only-with-the-right-to-set-it.done), the IDL fuzzer in CI [000-KRN-0069](../issues-done/000-KRN-0069-the-idl-fuzzer-in-ci.done), bus mastering off at boot [550-KRN-0070](../issues-done/550-KRN-0070-bus-mastering-off-until-a-driver-has-the-device.done), blockstore's quota [251-KRN-0071](../issues-done/251-KRN-0071-a-memory-quota-for-blockstore.done), the FAT free count [251-KRN-0072](../issues-done/251-KRN-0072-free-clusters-counted-a-fat-sector-at-a-time.done); QEMU's vvfat crash is [000-KRN-0067](000-KRN-0067-the-aarch64-boot-test-on-pcu.md))

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## A launch session holds as many grants as there are launch slots (211-APP-0044)

**Recorded by:** the tools track (APP), 2026-10-10, for [211-APP-0044](../issues-done/211-APP-0044-console-joined-to-the-shell.done).

### Problem

`loader`'s launch session keeps at most 5 grants (`Session.grants: [(u8, usize); 5]` in `loader/src/main.rs`). A sixth `grant` is refused with `limit`. `LAUNCH_SLOTS` (211-KRN-0058) names 16 slots a launcher may fill.

`console` started by `wm` now asks for the shell's commands too, so it may need six grants:

- the window;
- the user's files;
- system information;
- the camera;
- the shell's commands (`SLOT_SHELL`);
- for `record -w`, the window to see.

The `wm` suite's `record -w` lost its window lease to the limit. The tools track now grants `SLOT_SHELL` last and lets a start go on without it (wm and the shell), so the program runs without the shell's commands when the session is full.

### Plan (a proposal; the kernel track decides)

- **The session's array** holds `LAUNCH_SLOTS.len()` grants, or `SPAWN_GRANTS_MAX` less the loader's own.
- **A refused grant** still answers `limit`.

### Acceptance criteria

A launcher grants each of `LAUNCH_SLOTS` in one session and the program holds them all. `console` from `wm` gets the shell's commands beside the window lease of `record -w`.

## A console program ends when its console is gone (000-APP-0056)

**Recorded by:** the tools track (`APP`), 2026-10-10, from the walk through wm's menu in [000-APP-0056](../issues-done/000-APP-0056-every-entry-of-wms-menu-reacts.done).

### Problem

wm's menu has a category "Tests and performance": `check`, `bench` and `kbench` run there in a `console` window. Closing that window ends `console`, but not the program it ran:

- `mind::output::send` stops sending when the launcher's endpoint answers `Peer`, and the program goes on.
- A full `kbench` or `bench` then measures for minutes with nobody watching it, and takes processor time from whatever the user does next.
- `console` cannot end its programs. Ending another program is the shell's (process control), and the shell asks the user first.

000-APP-0056 asks that nothing started from the menu runs unseen. Until this is settled, the wm suite waits for these three to end before it closes their window (`RUN_TO_END` in `tests/qemu_smoke.py`).

### Plan (a proposal; the kernel track decides)

One of these:

- **In libmind.** A program whose launcher's endpoint is gone ends at its next output, with a nonzero status, as a Unix program does on a hang-up. A program that must outlive its console (none is known) could opt out.
- **In the three tools.** libmind says whether the output still reaches the launcher (`mind::output::open()`). `check`, `bench` and `kbench` look at it between rows, write the log they have, and say in it that they stopped because their console closed.

### Acceptance criteria

In QEMU, `kbench` started from wm's menu ends within one row after its window is closed, and the kernel's log shows it. The wm suite's walk then closes the window without waiting, and `RUN_TO_END` is empty.

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

## The aarch64 boot check waits for `[BLOCKSTORE] READY` on the serial line, and it no longer comes there

**Recorded by:** the tools track (`APP`), 2026-10-11, from its local gate (the branch at b518d36 merged with `main` at dde9418).

### Problem

The group "aarch64: boot and fault containment" (`tests/aarch64_smoke.py`) fails on `main` itself (dde9418, built for aarch64 on its own):

- It reads the serial line until `[INIT] READY` and then requires `[BLOCKSTORE] READY BLOCKS=0 NAMES=0 SECTORS=1/16384 CORRUPT=0 DAMAGED=0`.
- Since 251-STO-0013 the block store first logs `[BLOCKSTORE] INDEX: 4681 SLOTS (255 KiB) FOR 16384 SECTORS`; its `READY` line never reaches the serial line, neither before `[INIT] READY` nor in 180 s of waiting for it (1 CPU and 4 CPUs alike, three runs).
- In the QEMU suites on aarch64 the same `READY` line is there: the store suite reads it with `logs <blockstore's PID>`, and every store check passes.
- Most likely the mount now ends after `init`'s `READY`, and lines printed after that do not go to the serial line in this test, which starts no shell. The vvfat crash (000-KRN-0067) may have hidden this in earlier gates, since it fails the same group.

### Plan (a proposal; the kernel track decides)

- Check the block store's `READY` where its output is kept rather than on the serial line, or wait for it before `[INIT] READY` is taken as the end of the boot.
- If the line should reach the serial line, find why it does not since the index grows with the medium (with the storage track, `requests-STO.md` names this request).

### Acceptance criteria

The group passes on `main` and on a branch merged with it, with the block store's `READY` checked.

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
