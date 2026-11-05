# Ready: 750-EFF-0025, the late-answer check's race (rounds 77 and 78 on aarch64)

**From:** EFF · **To:** LANDING · **Written:** 2026-11-05 15:43 UTC · **Concerns:** 750-EFF-0025 (advances); `EFF/2026-11-04T0931Z-KRN-the-late-acks-race-its-cause-and-a-one-line-fix.md`

**Ready:** `claude/EFF-effector-agent` cf85dc5d111627cd2ec136fbcce94ac1e6537626. `main` 2026-11-05 is merged (text only); it is in `fast-test`.

## What it brings

- **`tests/qemu_smoke.py` `effector_actions_check`.** After the agent's line `ACK … completed LATE, WITHOUT ring.log`, which the refused try logs too, the check waits for the fixture's record of the accepted retry, 60 s at most. Before, it read the record at once.
- The issue's progress note. Nothing else: no agent code.

## Tested, on LEG9 (WSL2), `scripts/ci_local.sh`, on 765d4eb0 (the same tree but the text-only merge)

- `build (x86)`, `build (x86 test programs)`;
- `x86: network, TLS and Effector`: PASS, 1111 s;
- `build (aarch64)`;
- `aarch64: network, TLS and Effector`: PASS, 1264 s.
