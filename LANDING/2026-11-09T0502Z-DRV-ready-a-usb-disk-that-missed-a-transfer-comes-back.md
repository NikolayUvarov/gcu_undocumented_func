# DRV → LANDING: ready, a USB disk that missed one transfer comes back (211-DRV-0053)

**Ready:** `claude/ASR-DRV` ada98444

**On `main`** 6eed1563 (a fast-forward from it).

## What it changes

**Advances** 211-DRV-0053 (stays open until the MacBook Pro shows the recovery). What the maintainer saw on 2026-11-09: the MacBook Pro's boot stick was lost for good after ~15 h, every file request failed, and its disk was written all the time.
- **`usb_host`, xHCI bulk transfers:**
  - a transfer with no answer stops the endpoint and moves its dequeue pointer past the TRB left behind, as endpoint 0 already did;
  - any other error resets the endpoint on both sides, as a stall did, and is logged.

  Before, the next transfers waited behind the stuck TRB or failed on the halted endpoint.
- **`usb_storage`:** before claiming again after a failed BOT cycle, it sends a Bulk-Only Mass Storage Reset and gives the old claim back. Kept, the interface could never be claimed again, so the device was "gone" until a reboot.
- **`bcm_wifi`:** its 10-second traffic tally (550-DRV-0036) is logged only for a period with a loss: a frame not acknowledged, a DMA error, the ring full, or five beacons or more missed in a row. The quiet periods are summed in one line every 10 minutes. Its four lines every 10 s kept the system log saving to the stick every 11 s.

The issue file and `issues/README.md` come with it.

## What was tested, where

On this machine, the working tree of ada98444 before its last change (that change is `bcm_wifi`'s loss criterion only, checked by `cargo build` of `bcm_wifi` after it):
- `./02_build.sh`: success, no warnings in the three crates;
- `scripts/ci_local.sh --only x86`, all PASS:
  - `build (x86 test programs)`;
  - `x86: keys, shell, tools` (the `usb` and `ehci` suites);
  - `x86: files and block writes` (the block suite through ATA, AHCI and USB);
  - `x86: USB image` (the disk unplugged, back, another medium);
- the whole `scripts/host_tests.sh` with `MIND_SSHD`: 193 suites, none failed.

**On the MacBook Pro:**
- `bcm_wifi` with the tally change, built on its own release tree 101b4f47, replaced live over the link. It rejoined CoolNet2 in 3 s, and logged no tally line in its first 90 s.
- `usb_host` and `usb_storage` were not run there: they need a slot update.
- Nothing can make the stick miss a transfer at will, so the recovery path itself is not yet seen on hardware (MC-12.1, 12.2).
