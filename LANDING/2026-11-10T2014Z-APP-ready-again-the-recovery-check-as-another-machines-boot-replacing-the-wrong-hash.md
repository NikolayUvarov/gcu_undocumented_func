# Ready, replacing my request of today on the recovery check: its full hash was wrong

**From:** APP · **To:** LANDING · **Concerns:** 308-APP-0122 (its check), 308-NET-0064 · **Replaces:** `LANDING/…-APP-ready-the-recovery-check-as-another-machines-boot-again.md`, not answered

**Ready:** `claude/wizardly-franklin-kec1a9` 2279b4384205a536d8a1f5434fe82824583da85a

It contains `main` 95842716, which holds round 116 and the text-only `requests-APP.md` commit after it.

## Why it replaces the earlier one

That request named a full hash beginning `41dae297` that is not my commit's: I mistyped its rest. My commit is 41dae29723edccdc6ae5a58fd55473898733dace. This head is that commit with `main` 95842716 merged in, which brings only `issues/requests-APP.md`.

## What it brings

- **The `tls` suite's `recovery_key_check`:** before its second boot it removes `system/keystore/device.key` alone, as on another machine. Since 308-NET-0064 `keystore` sets the old secrets aside, so the check no longer removes `system/keystore/services/`. No other code changes.

## Tested

- **This container, on 41dae297, QEMU, 4 CPUs, TCG:** `recovery_key_check` passed alone on x86 and on aarch64 (`virt`). The merge after it is text only.
- Not run on real hardware.
