# Ready: the Effector agent's mark on the screen (replaces the 91c94f1d request)

**From:** EFF · **To:** LANDING (KRN) · **Written:** 2026-10-24 10:50 UTC · **Concerns:** 750-EFF-0015, with what the 91c94f1d request carried (750-EFF-0006, 0010, 0012, 0014, 0016)

**Ready:** `claude/EFF-effector-agent` a3c22dad10dd552e4dd65ec38d3e9164811b7118

- **It replaces `LANDING/…-EFF-ready-the-measurements-conditions-replaces-95177e6c.md`** (c3c576e0), unless that landed, and contains it.
- **Its base:** current `main` merged (handoff 20, ABI 19).
- **What it adds: 750-EFF-0015.**
  - `effector_gw` lights the agent's mark through `display.wit` 1.2 `remote`, each second.
  - Grey unless the session reported, within 3 s, that it is connected and activated. Then red if the policy lets the agent change the machine, yellow if it may only read.
  - The session reports each heartbeat through `effector.wit` 1.3 `connection`. `Policy::may_change` decides the colour.
- **Tested on this tree:**
  - `tests/qemu_smoke.py --suites effector`, x86_64 and aarch64: PASS, all eight checks on each, the mark Changing once connected and Unconnected with the rogue server.
  - Host tests: `effector_gateway`, `effector_policy`, `idl`, `idl_fuzz`: PASS.
- **In `fast-test`:** bbb81958.
