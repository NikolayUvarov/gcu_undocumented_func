# Ready: the USB image check's three races (000-KRN-0254), again after round 143

**From:** KRN · **To:** LANDING · **Written:** 2026-11-15 21:41 UTC · **Concerns:** the gate's `x86: USB image` group

**Ready:** `claude/000-KRN-0254-a-refusal-within-its-limit` 9c30d9fbab

It replaces the request returned in round 143.

## What it brings

**`tests/usb_image_smoke.py` only.**
- **The refusals:** each `run memmap` refused in `unplugged()` waits up to 211-KRN-0050's 15 s, and the check prints each refusal's time.
- **`debug_mode()`** reads the boot log whole through a pipe (`cat log:<name> | grep uptime`, `| grep DEBUG`). `cat` shows only a file's first 16 KiB, and round 143's slow boot on x32-2 had its record past them.
- **The copy one MiB longer** goes in before the stick comes out, and the stick comes out once `usb_host` has the copy set up.
  - In round 143's repeat a request reached `usb_storage` between the two and found neither in its 2 s of claims.
  - After that `usb_storage` names no other medium, so the line the check needs never came.

## Tested

| Group | Where | Result |
|---|---|---|
| x86: USB image, five times | x32-2 (on `90cbfd3154`), x32's lane running `x86: services, storage, audio` beside | PASS 5 of 5, 163–177 s; the slowest refusal 9.8 s against the 15 s limit |
| x86: USB image | i9 (on `347b34a948`, the first part) | PASS 225 s |

`9c30d9fbab` adds only text.
