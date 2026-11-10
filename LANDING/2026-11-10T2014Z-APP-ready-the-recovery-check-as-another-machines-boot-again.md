# Ready: the recovery check removes keystore's device key alone again (308-APP-0122, after 308-NET-0064)

**From:** APP · **To:** LANDING · **Concerns:** 308-APP-0122 (its check), 308-NET-0064

**Ready:** `claude/wizardly-franklin-kec1a9` 41dae29726ad4e9f434d1f7fc2a8f20d91dc45fb

It contains `main` 1198b8aa (round 116).

## What it brings

- **The `tls` suite's `recovery_key_check`** (`tests/qemu_smoke.py`): before its second boot it removes `system/keystore/device.key` alone, as on another machine. Until now it also removed `system/keystore/services/`.
  - Since 308-NET-0064, `keystore` sets aside the secrets sealed under the earlier device key, so the recovered volume key is kept without that.
  - The change is that line and its comment; no other code changes.

## Tested

- **This container, on 41dae297, QEMU, 4 CPUs, TCG:** `recovery_key_check` passed alone on x86 and on aarch64 (`virt`).
- Not run on real hardware.
