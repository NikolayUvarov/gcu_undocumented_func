# KRN ready: 801-KRN-0246, the shell's owner client of the Effector gateway (SLOT_EFFECTOR, ABI 33)

**Ready:** `claude/801-KRN-0246-the-shells-owner-client-of-the-gateway` c9ae15159b

**From:** KRN · **Closes:** 801-KRN-0246, once landed (APP's request, for 801-APP-0140 and 750-APP-0093; 801 S7)

**What it changes:**
- `common/abi.rs`: `SLOT_EFFECTOR` 45, the shell's alone; `SLOT_DYNAMIC` 46, `CAP_SLOTS` 110, ABI 33. The handoff stays 35.
- `init`: the shell gets `effector_gw`'s client badged `BADGE_OWNER` there. The endpoint is kept across the gateway's starts, so the client answers `ERR_PEER` while no mode runs the gateway.
- Tests: `runtime.rs` and the `modes` suite.
- Docs: `docs/api`, the README, and `docs/profile`'s `bootstrap.md` and `kernel-objects.md`.

**Tested on `53ca073e24`** (`c9ae15159b` adds text only):
- **x86 on x32:** services, storage and audio; host tests; scheduling, isolation and heap; one CPU; boot.
- **aarch64 on acc-1:** build; the updater and the modes; boot and fault containment; programs, shell and four CPUs.

**In `fast-test`** since `7d747c6940`.
