# KRN ready: the updater's period from the release it booted (351-UPD-0036, for LNK)

**Ready:** `claude/351-UPD-0036-the-slots-period` 24445c514

**From:** the kernel session (KRN, PRT, NET, UPD)

**What it lands** (on `main` 952a4e07): a release may carry `period.txt` in its slot, listed in its signed manifest. Then the updater checks as often as it says, in place of `update.txt`'s `every`.

- **What it holds:** one `every SECONDS`. A `source`, `pin` or any other line refuses the file.
- **What the updater checks first:** the slot's manifest must be the one booted, and the file must match its line in it.
- **What stays the root's alone:** `source`, `pin`, `channel`, `automatic` and `tries`.
- **Staging and applying:**
  - `fetch` stages it with the slot's files.
  - `apply` checks it.
  - The block store keeps it.
- **The build** writes it with `MIND_UPDATE_EVERY`. `sign_manifest.py` lists it, and `make_usb_image.py --slots` and `boot_slots.py layout` put it in slot A.
- **Docs:** `docs/update/updater.md`.

**Tested** at 3fa050ae. Since then only the issue's text changed.
- **acc-1, through Effector:** "x86: the updater and an update through the link", passed in 835 s.
- **acc-2, through Effector:** "aarch64: the updater and the modes", passed in 751 s.
- **The `update` suite's checks:**
  - release 5's file, naming a source, is refused;
  - release 6's `every 7` is used in slot B, with checks under 18 s apart against the root's 20;
  - slot A's file, changed on the disk, is not read.
- **The kernel session's container:**
  - `tests/updater_host.rs`: 17 tests passed;
  - `tests/usb_image_test.py`: passed.

**What changes for other suites:** nothing, unless a volume carries `period.txt`. The other users of `update_releases` give none.
