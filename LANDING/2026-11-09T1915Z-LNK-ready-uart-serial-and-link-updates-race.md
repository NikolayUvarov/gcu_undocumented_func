# LNK → LANDING: ready, `uart_serial` over a board's UART (100000-LNK-0038), and `link_update.py`'s race (800-LNK-0040)

**Ready:** `claude/LNK-link` 251d1efd

**On `main`** 9cd836e3, merged in; it brought only `tests/qemu_smoke.py` and issue text.

## What it changes

- **db50334a, 100000-LNK-0038: `uart_serial`, a new ring-3 service.** It serves `idl/serial.wit` over the UART in `SLOT_DEV0`:
  - 16550 at ports (chip 3): the divisor from 1.8432 MHz;
  - PL011 (4) and the kernel's console channel (5): 115200 only;
  - the line's interrupt when `SLOT_IRQ` holds one, a 2 ms poll besides;
  - a 64 KiB receive queue.

  `serial.wit` 1.1 changes documentation only: the UART chips, and what `invalid` and `failed` mean there. `usb_serial` and `link` are unchanged, and there is no new type. `uart_serial` is in `02_build.sh`; `BOOT_FILES` and `init` are KRN's 100000-KRN-0221, which waits for it.
- **b9fa8183, 800-LNK-0040: `link_update.py`** no longer ends an update on an `Older` refusal of a check that read another version. That check ran while the channel was being put, read the running version's channel, and stopped the MacBook Pro's update today, though the updater's next check fetched the release. A refusal of a check that read this version still ends it, with a test.
- Docs: `docs/link/mindlink.md` and `mindlink_RU.md`, a section on the board's UART. Issue text: 100000-LNK-0038's progress.

## What was tested, where

On this machine, on db50334a (base `main` 31a3e995, ABI 29):
- `./02_build.sh`: `uart_serial` without warnings, and also for `aarch64-unknown-none-softfloat`;
- the whole `scripts/host_tests.sh` with `MIND_SSHD`: 196 suites, none failed;
- `scripts/ci_local.sh --only x86`, all PASS:
  - `build (x86 test programs)`;
  - `x86: services, storage, audio` (`link` over the emulated FT232, and `modes`);
  - `x86: USB image`.

b9fa8183 on top: `tests/link_update_test.py` passed, its new case failing without the fix.

**Not run anywhere yet:** `uart_serial` itself, which needs 0221's `init`. The fixed `link_update.py` has not driven an update yet; the Mac's release of today went in with the old script and the updater's next check.
