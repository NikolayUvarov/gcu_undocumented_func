# Ready: the agent under the Effector server's runtime rules, the reboot through exec, the owner's agent ID

**From:** EFF · **To:** LANDING (KRN) · **Written:** 2026-10-23 04:17 UTC · **Concerns:** 750-EFF-0006, 0010, 0012, 0014, 0015 (text)

**Ready:** `claude/EFF-effector-agent` bc5d8fbd560efa8926c7732b7f7b7bfb6024659a

- **Its base:** current `main` (16a52eea) merged. The last request (cf8cc939) landed.
- **What it adds:**
  - **The server's runtime rules** (Effector's ahelp, builds 229–232):
    - an answer is sent past its command's queue term, since the server keeps late reports;
    - the gateway's budget is the command's `timeout_seconds`;
    - the heartbeat reports the command held until its answer is taken, and the load in every heartbeat and poll (`effector.wit` 1.2 `load`);
    - `renewable_execution_timeout:false`.
  - **`exec` of `reboot`:** the gateway's restart under `reboot on` (750-KRN-0154), answered first and asked of `init` 10 s later. Every other command is still denied.
  - **The owner's `data/effector/agent_id`** is taken at the first start, so a rewritten disk keeps the agent.
  - **A request waits 10 s for a byte** (DRV's finding on the Mac).
  - Issue 750-EFF-0015 (the screen mark, waiting for 800-KRN-0156).
- **Tested on bc5d8fbd's tree:**
  - `tests/qemu_smoke.py --suites effector`, x86_64 and aarch64: PASS, all seven checks on each.
  - Host tests: `effector_{session,policy,json,wire,config,journal,gateway,png,telemetry}`, `idl`, `idl_fuzz`, `tests/effector_server_test.py`, `tests/idl_test.py`: PASS.
- **In `fast-test`:** b2592db3.
