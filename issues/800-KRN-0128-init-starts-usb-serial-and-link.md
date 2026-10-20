# 800-KRN-0128 — `init` starts `usb_serial` and `link` when the boot volume holds `link.conf`

**Type:** `init`, ABI (`common/abi.rs`: two boot images) · **Owner:** kernel session · **Priority:** P2 · **Status:** made on the kernel branch with `claude/LNK-link` merged, in `fast-test` (2026-10-20); the gate is left · **Blocked by:** the link track's `usb_serial` and `link` in `main` (`claude/LNK-link`) · **Main task:** 800 (the link track's) · **Constitution:** MC-2.3, MC-3.4, MC-3.7

## Problem

The link track's requests of 2026-10-20 (`internal-dev-messages`, `KRN/2026-10-20T0706Z-LNK-…` and `KRN/2026-10-20T0845Z-LNK-…`):
- `usb_serial` ([800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md)) and `link` ([800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md)) build. They ran in QEMU with a local patch of `init`.
- The drivers track's part is done (800-DRV-0034): `usb_host` gives a converter to a client badged `mind::usb::BADGE_SERIAL`.
- Only `init` can start the two services and hand out their clients, and neither is a boot image.

## Plan

1. **Two boot images.** `usb_serial` after `usb_hid`, and `link` after `vfs_server` and `logd`. That is an ABI change: ABI 13, since ABI 12 (351-KRN-0115, 0116) went ahead without them while the link track's code is not in `main`.
2. **Started only when the boot volume's root holds `link.conf`,** as `autorun.msh` is read: only whoever writes the disk can put it there. Otherwise both are `NOT STARTED: NO LINK.CONF`.
3. **`usb_serial`'s grants,** as `usb_storage`'s and `usb_hid`'s: `usb_host`'s client badged `BADGE_SERIAL` in `SLOT_DEV0`, and its server endpoint in `SLOT_SERVICE`.
4. **`link`'s grants:**
   - `usb_serial`'s client in `SLOT_DEV0`. `link` is its only holder (MC-2.3).
   - A writable `vfs_server` client confined to `data/link/` in `SLOT_VFS`, from 750-KRN-0120. The directory is made if it is missing. An ordinary user client is not shipped in its place.
   - A log reader (`BADGE_READ`) in `SLOT_LOG`.
   - Its arguments are `link.conf`'s words.
5. **No console, network or other files.** The console comes later, with the tools track.

## Progress (2026-10-20)

The link track asked for this part at once, on its branch and into `fast-test`, so that the link carries a machine's log from the start of the boot. Made on the kernel branch with `claude/LNK-link` (c658337) merged:

- **ABI 13.** `usb_serial` and `link` are boot images right after `vfs_server` (`BOOT_IMAGES` 41), so the link starts before the drivers and services that most need debugging. Both are built for x86 and aarch64 (`02_build.sh`, `scripts/build_aarch64.sh`).
- **`link.conf` is read by the bootloader,** through the firmware, from the root of the volume it was loaded from. `BootInfo.link_present` and `link_words` (at most 64 bytes, the file's words joined by single spaces) follow the images, so ABI 10's prefix and the version's offset stay. So the link starts even when MIND Core cannot read its own boot disk. Without the file `init` logs `usb_serial NOT STARTED: NO LINK.CONF` and the same for `link`.
- **Grants** as in the plan:
  - `usb_serial`: `usb_host`'s client badged `BADGE_SERIAL` in `SLOT_DEV0`, its server endpoint in `SLOT_SERVICE`;
  - `link`: `usb_serial`'s client in `SLOT_DEV0`, a log reader in `SLOT_LOG`, `link.conf`'s words as arguments, and in `SLOT_VFS` a writable client confined to `data/link/` that `init` asks `vfs_server` for at each start (750-KRN-0120's `scope`, the directory made if missing). Without the boot volume `link` gets no file client and `init` logs `link: NO FILES (…): THE LOG ONLY`.
- **The log from the kernel's first line.** Right after `logd` starts, `init` copies the kernel's lines (the hardware report's `Kernel lines`) into the system log. `logd`'s ring grows from 256 to 2048 records, and `init` is not rate-limited, so a whole boot fits. `mindlink log` reads it all.

## Acceptance criteria

- [ ] A QEMU suite with `-device usb-serial` on a pseudo-terminal and `link.conf` on the test disk, written by the link track:
  - `mindlink` moves a file each way and reads the log;
  - a name outside `data/link/` is refused by `vfs_server`, not only by `link`.
- [ ] Without `link.conf`, neither service starts, and every other suite passes as before.
- [ ] `stat caps` of `link` shows exactly the grants above.

## Related

[800-LNK-0002](800-LNK-0002-usb-serial-ftdi-and-ch340.md), [800-LNK-0003](800-LNK-0003-the-link-service-on-mind-core.md), [750-KRN-0120](750-KRN-0120-writable-scopes-for-a-service.md), [351-KRN-0115](../issues-done/351-KRN-0115-the-ssh-service-in-the-boot-services.done) (ABI 12, without these), [docs/msh.md](../docs/msh.md) (`autorun.msh`, read the same way).
