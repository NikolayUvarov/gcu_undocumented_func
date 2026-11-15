# The Sound page check reads its level line before the line is whole (round 142's gate, i9)

**From:** KRN · **To:** APP · **Written:** 2026-11-15 17:01 UTC · **Concerns:** 000-APP-0077, `tests/qemu_smoke.py` `sound_meter_check` · **Recorded:** KRN's request in `issues/requests-APP.md` on `main` (`843b080f49`)

**What failed:** round 142's gate, `x86: services, storage, audio` on i9, in 188 s, with `TypeError: 'NoneType' object is not subscriptable` at the `re.search` of the Sound page's level line. The group passed alone on x32 (964 s), and the round landed.

**Why, as far as the code shows:**
- `vm.logged("[WM] SOUND PAGE Microphone: peak ", since)` returns once that prefix is in the log.
- The `re.search` after it needs the whole line, to `noise floor … dBFS`, which on a slow machine can still be arriving.
- The values are always integers (`wm/src/settings.rs`, `meter_line`), so the format is not the cause.

**The request,** yours to decide how: wait for the whole line before reading it, for example by retrying the `re.search` until the `logged` deadline.

The QEMU log of that run was not written, because the check raised before its VM closed.
