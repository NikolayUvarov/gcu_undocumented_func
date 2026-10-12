# 211-KRN-0090 — `init` starts the battery service; `SLOT_BATTERY` and `REQUEST_BATTERY` (ABI 8)

**Type:** kernel ABI (`common/abi.rs`), `init`, `libmind` `process` · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress (made, tested in QEMU; the MacBook Pro's run left) · **Blocked by:** — · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md) · **Constitution:** MC-12.4

## Problem

[211-PRT-0008](211-PRT-0008-the-battery-behind-the-embedded-controller.md) adds the `battery` service. It needs three things:

- a boot image;
- `init` to find its embedded controller and SMBus host and start it;
- a fixed slot through which the shell lends its client to programs, `wm` first, for the maintainer's indicator.

Slots 1 to 31 were all named.

## Plan

- **`common/abi.rs`:**
  - `battery` is a boot service after `gpio` (`BOOT_IMAGES` 34);
  - `SLOT_BATTERY` is 32, and `SLOT_DYNAMIC` moves to 33;
  - `LAUNCH_SLOTS` holds `SLOT_BATTERY`;
  - `CAP_SLOTS` is 104, so 64 or more slots of a new table stay dynamic, as `tests/runtime.rs` asks;
  - `ABI_VERSION` is 8: a program built for the new layout cannot start on a kernel that refuses grants at 32 (MC-12.4).
- **`libmind::process`:** `REQUEST_BATTERY` is bit 21; bit 20 is the tools track's `REQUEST_SHELL`.
- **`init`:**
  - `hardware::battery_place` reads the ECDT and the DSDT and SSDTs while init holds the platform privilege;
  - `battery` gets the two ports and `smbus=0xNN` (a plan now carries arguments: `name\0arguments`);
  - without them it is not started (`NO DEVICE`);
  - the shell gets a client in `SLOT_BATTERY`.
- **The lending to programs** (the shell, `wm`) is the tools track's: `requests-APP.md`.

## Acceptance criteria

- `tests/runtime.rs` checks the slots, `CAP_SLOTS` and ABI 8.
- QEMU x86 and aarch64: `battery` is not started, and the suites pass.
- The MacBook Pro: `battery` runs and the shell holds its client.

## Related

[211-PRT-0008](211-PRT-0008-the-battery-behind-the-embedded-controller.md), [211-KRN-0058](211-KRN-0058-slots-for-the-shell-and-the-clipboard.md) (the last slots added), [docs/api/README.md](../docs/api/README.md).
