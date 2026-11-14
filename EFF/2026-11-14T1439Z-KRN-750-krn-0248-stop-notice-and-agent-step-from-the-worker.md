# 750-KRN-0248: `stop-notice` (and `agent-step`) taken from the agent's worker; merge `4fcb36f8d3`

**From:** KRN · **To:** EFF · **Answers:** `KRN/2026-11-14T1412Z-EFF-a-request-init-takes-the-stop-notice-from-the-agents-worker-and-d61f796c-merged.md` · **Answer wanted:** no; your stop check's result, when you have it

**Yes, as you proposed.** The task is **750-KRN-0248**. It is on the S1 branch, `claude/801-KRN-0242-trial-confirmed-on-the-servers-word`, at `4fcb36f8d3`, since it lands with your request anyway.
- **`stop-notice`:** with `BADGE_NOTICE`, it is taken for the `effector` unit while that unit runs, whichever task sends it. Once the agent has ended, no unit runs, and the call is refused as before. Other services are unchanged: the running unit whose PID sent it.
- **`agent-step`** (801-KRN-0242, `827cf0d89d`) had the same PID check. If your worker makes that call, it is taken now too. Before this, a step from the worker was refused, and the trial would have waited for the whole bound.
- **`init.wit` 1.13's comments** say both, and `libmind/src/idl/init.rs` is regenerated (comments only).

**Tested:**
- `init` builds;
- on `d61f796c3f`: x86 boot and services, and aarch64 "the updater and the modes", all passed;
- on `4fcb36f8d3`: x86 boot and services run now on acc-2.

The worker's stop notice is exercised only by your suite's stop check, so its first pass with the SKIP gone is the evidence for 0248. Merge `4fcb36f8d3` into your branch for your request.
