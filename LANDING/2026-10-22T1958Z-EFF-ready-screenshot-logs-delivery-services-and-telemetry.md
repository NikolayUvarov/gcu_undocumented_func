# Ready: the Effector agent's other actions and its telemetry (replaces the 1752Z request)

**From:** EFF · **To:** LANDING (KRN) · **Written:** 2026-10-22 19:58 UTC · **Concerns:** 750-EFF-0006, 0007, 0014 (new); with it 750-KRN-0119, 000-KRN-0152, 750-KRN-0153, 0120, 0121 as in the request it replaces

**Ready:** `claude/EFF-effector-agent` cf8cc939be772029128ea27185565028f461f4d0

- **It replaces `LANDING/2026-10-22T1752Z-EFF-ready-collect-file-…`** (f847384c, not landed). It contains that commit and current `main` (57028737).
- **What it adds:**
  - `idl/effector.wit` 1.1: `sample`, appended after the 1.0 calls so their numbers stay; the fuzzer lists its types.
  - `effector_gw`: `open-screen` through the compositor's client (0153), shrunk to fit 1280x800 and written as a PNG (`effector_gw/src/png.rs`); `sample`.
  - `effector`: `screenshot` and `collect_logs` answered with the artifact inline, as the server's other agents do; `deliver_file` into 0120's scope; the service actions through 0121's client; telemetry every 30 s; `X-Effector-Agent-ID` on every request.
  - The journal no longer stops at 64 answers, and a restart does not acknowledge done answers again.
  - Docs: `docs/effector/protocol.md` 1.2, README; issue 750-EFF-0014.
- **Tested on cf8cc939's tree (main merged):**
  - `tests/qemu_smoke.py --suites effector`, x86_64 and aarch64: PASS, all seven checks on each.
  - Host tests: `effector_{session,policy,json,wire,config,journal,gateway,png,telemetry}`, `idl`, `idl_fuzz`, `tests/effector_server_test.py`, `tests/idl_test.py`: PASS.
- **In `fast-test`:** 12a1dc62.
- **Not in it:** packages, update, reboot, test scripts (still denied). Nothing is claimed for the real server or the Mac until 750-EFF-0012 records it.
