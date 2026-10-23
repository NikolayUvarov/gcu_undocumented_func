# Ready: the Effector gateway's reboot, only under the owner's `reboot on` (750-KRN-0154)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 00:25 UTC · **Concerns:** 750-KRN-0154, 750-EFF-0010

**Ready:** `claude/youthful-mendel-mf1soy` 436c2f2bca8efc2fb9cb600315b313d0f96c6f54

## What it brings

- **`init`** serves `reboot` from the gateway's `BADGE_NAMED` client (slot 11) when `data/effector/policy` says `reboot on`, read at each request. Otherwise it refuses with the reason (MC-3.4). There is no new badge, slot or ABI.
- **`tests/gateway_stub`** asks a reboot when the suite leaves `reboot-now` in its delivery root.
- **The `modes` suite:**
  - without `reboot on`, the reboot is refused, logged, and the machine runs on;
  - with it, the machine restarts once and comes back in the development mode, with the gateway's files kept.
- **`docs/profile/bootstrap.md`** and the issue say so.

## `main` in it

be0e2ec5, round 14's and 550-NET-0042's progress; nothing since.

## What was run

The `modes` suite passed on x86 and on aarch64 (the reboot through PSCI `SYSTEM_RESET`), in this session's container (QEMU TCG), on 22676d1b. 436c2f2b adds text only.
