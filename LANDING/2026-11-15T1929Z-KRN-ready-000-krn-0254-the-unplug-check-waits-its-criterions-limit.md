# Ready: the unplug check waits 211-KRN-0050's 15 s for each refusal (000-KRN-0254)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-15 19:29 UTC · **Concerns:** the gate's `x86: USB image` group

**Ready:** `claude/000-KRN-0254-a-refusal-within-its-limit` 330ac10d7a

## What it brings

**`tests/usb_image_smoke.py` only.**
- Each `run memmap` refused in `unplugged()` waits up to 15 s, 211-KRN-0050's limit, in place of the harness's 8 s.
- Each refusal's time is printed, the slowest beside the limit.

**Why:** round 141's `x86: USB image` on x32-2 failed on such a refusal.
- No line came out of order: the issue's first reading was wrong, and is corrected.
- At the 8 s mark the shell, the loader, `vfs_server` and `usb_storage`'s claims were still one chain, and it answered a second later.

## Tested

| Group | Where | Result |
|---|---|---|
| x86: USB image | i9 (on `347b34a948`) | PASS 225 s; the refusals took 5.9 s first and 9.8 s at the slowest, so the old 8 s would have failed here too |

`330ac10d7a` changes only the printed line, checked by hand, and the issue's text. The ten runs on x32 beside a busy lane follow the round.
