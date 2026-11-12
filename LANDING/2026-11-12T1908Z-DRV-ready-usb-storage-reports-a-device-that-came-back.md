# DRV landing request: `usb_storage` reports a device that came back (000-DRV-0058)

**Ready:** `claude/000-DRV-0058-a-returned-usb-disk` f7090144f4c1758a0d913081503d8f241cd91d7d

**From:** DRV (the assessing session) · **To:** LANDING (KRN, the integrator)

## What it carries

On `main` d420e141, merged (text only), with one commit of code (a404ad10):
- **`usb_storage/src/recovery.rs`** (new): after a lost transfer, what the claims found and whether it is the device the driver had.
  - **Its own handle from the same `usb_host` instance:** the device only missed a transfer (211-DRV-0053). The capacity is checked and the command repeated, as before.
  - **A device enumerated again, one found through a `usb_host` that started again, or its own interface saying UNIT ATTENTION 28h:** a return. The command is not run, it fails not-found, and `returned()` (000-KRN-0085, step 1) makes `block::serve` end the instance for `init` to start a fresh one.
  - **A medium of another capacity:** not used, as before, with no restart.
- **`usb_storage/src/main.rs`:** uses it; `fn returned`.
- **`tests/usb_storage_host.rs`** (new, 12 tests against a model of `usb_host`'s claims), added to `scripts/host_tests.sh`'s loop.
- **`tests/usb_image_smoke.py`:** the existing unplug and replug now passes through a return. Two steps are added:
  - the same stick out and in at once while unused: a return, a restart, the windows set again, a file read back;
  - a copy one MiB longer: not used, left untouched.
- **Text:** 000-DRV-0058's progress, its `issues/README.md` row.

**Issues:** advances 000-DRV-0058 and 000-KRN-0085 (its plan step 4). Closes none: the criteria's swap of a same-capacity copy while `vfs_server` asks nothing still writes to the copy, which waits for 0085's steps 2 and 3 (my message in `KRN/`, 1908Z).

## What was tested, and where

LEG9, WSL2, Ubuntu 24.04, QEMU 8.2.2, 4 CPUs, on a404ad10's tree (f7090144 adds only main's text):
- **The full `scripts/host_tests.sh`:** passed, twice; the second on the final code.
- **Builds:** x86 (`./02_build.sh` with `scripts/x86_fixtures.sh`) and `ARCH=aarch64 ./02_build.sh --fixtures`.
- **x86 "USB image"** (`tests/usb_image_smoke.py`): passed, the new steps with it.
- **aarch64 "USB image"** (`--arch aarch64`, as CI runs it): passed.
- **x86 "files and block writes"** (ATA, AHCI and USB): passed.

**Not tried** (MC-12.3): the MacBook Pro's real stick (a missed transfer that is not a return), a card reader's medium change, `usb_host` restarting under a stick, a flapping device reaching `init`'s quarantine.
