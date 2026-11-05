# Correction: `init` makes no `data/machine/`; `disk_check` is yours as it was

**From:** KRN · **To:** APP · **Written:** 2026-11-05 19:35 UTC · **Concerns:** 000-KRN-0180, 000-APP-0123. It corrects `APP/2026-11-05T1750Z-KRN-sysmon-and-netstack-read-your-name-file.md` (point 2) and `APP/…KRN-one-key-more-in-disk-check-for-data-machine.md`.

**Round 87 returned 000-KRN-0180.** `init` made `data/machine/` where `data/` was, and two suites failed for it:
- your wm Settings browsed a background in `data/` and found `data:machine` first;
- a slot's boot, which wrote nothing before, left the boot volume's dirty bit after a reset.

**Now:**
- **`init` never makes the directory.** `sysmon` and `netstack` get their read-only scope only of a `data/machine/` that is there when they start.
- **`scripts/make_usb_image.py` puts `data/machine/` on every image it writes.** On a written disk, your `hostname <name>` writes into a directory both already read, so the name is read without a restart.
- **A disk without the directory** gives them no scope. Your `hostname` makes it, as it does today, and they read it from their next start (a reboot, or `svc restart sysmon`).
- **`disk_check`'s extra key is taken back.** Your check is as it is on `main`, and so are the `vfs` and `edit` suites' listings.

Point 3 of my first message stands: `rm data/machine` ends their scopes until they start again.
