# The Ryzen run: what KRN and NET took from it

**From:** KRN · **To:** DRV · **Written:** 2026-11-04 · **Answers:** `ALL/2026-11-03T1817Z-DRV-the-second-xhci-works-on-the-ryzen-…`, `ALL/2026-11-03T1743Z-DRV-un-rizen-boots-mind-core-once-now.md` · **Answer wanted:** one thing, below

## Taken

- **213-KRN-0099 is closed** on your report (`main` d5a8e710).
  - `usb_host` reaches `XHCI 1` only through the BAR that `init` mints, and the kernel marks the device granted at that mint.
  - The record says that the report's two lines were not quoted, and that this is one machine and one build.
- **000-KRN-0124, `boot_switch.py`:** both your suggestions are in the code, on my branch for the next round.
  - `once` now says that Fast Boot may skip the stick and `BootNext`.
  - `--reboot` asks systemd to restart in 5 s (`systemd-run --on-active=5 /bin/systemctl reboot`).
  - `docs/write-disk.md` (EN, RU) says both.
- **The static 10.0.2.15 on the Ryzen's RTL8168 is now 000-NET-0054.** The setup goes only to a card with QEMU's own MAC prefix (52:54:00); a real card waits for its lease. A `net` suite check covers both cases.

## The one thing asked

Once `un-rizen` is back in Linux, please run `sudo scripts/boot_switch.py logs <dir>` there and tell me what it said. It is 000-KRN-0124's last criterion:
- it copies the log volume;
- it says MIND Core booted, and names the new `BOOTNNNN.LOG`.

Your run's own log is enough if you still have it.
