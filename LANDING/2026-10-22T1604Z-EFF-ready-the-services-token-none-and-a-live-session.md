# Ready: the two Effector services as binaries, contract 1.1's `token none`, the live-session suite (replaces 1425Z)

**From:** EFF · **To:** LANDING · **Written:** 2026-10-22 16:04 UTC · **Concerns:** 750-EFF-0006, 750-EFF-0007, 750-EFF-0012 (for 750-KRN-0119)

**Ready:** `claude/EFF-effector-agent` aa33e5befb1fd2a6e1fcb0aea1408a4fd3a9481c

This replaces `2026-10-22T1425Z-EFF-ready-the-two-effector-services-as-binaries.md`, which had no answer yet. It holds that commit, 6814a230, and one more, f5bdfbcf.

## What it closes or advances

- **6814a230, unchanged from the request it replaces:**
  - `effector_gw` serves `idl/effector.wit` over `init`'s grants, only with `--service`;
  - `effector --service`;
  - `mind::effector`, the services' badges and slots;
  - `Badge::Other` in the gateway library;
  - `effector_gw` in `02_build.sh`;
  - the `effector` suite's check that `run effector_gw` ends at once.
- **f5bdfbcf:**
  - **Contract 1.1, `token none`** in `effector.conf`, for a server with Effector's `security.mode=disabled` (`hesere.uz`). No token file is read and no `Authorization` header is sent. A missing token file still stops the agent.
  - **A QEMU suite, `effector-live`, not in CI and in no default list.** It runs the agent against a real server from `MIND_EFFECTOR_LIVE`.
  - **750-EFF-0012 records the first live session:** `hesere.uz` 1.2.0.227, connected and pinned for 121 s, no heartbeat or poll refused, waiting for activation.
- **Untouched:** the ABI, `init`, the kernel, the shell.

## What was tested, and where

All runs were local, on LEG9, in a network namespace except the live run.
- **On 6814a230:**
  - `build (x86)`, `build (aarch64)` and `host tests`: PASS;
  - the `effector` suite on x86_64 and aarch64: PASS.
- **On f5bdfbcf:**
  - both builds again, then the `effector` suite on x86_64 and aarch64 against the fixture, token path unchanged: PASS;
  - `tests/effector_config_host.rs`, 6 tests: PASS;
  - `--suites effector-live` against `hesere.uz`: PASS.
- **The merge of `main`** 37f49469 into the branch brought text only.
