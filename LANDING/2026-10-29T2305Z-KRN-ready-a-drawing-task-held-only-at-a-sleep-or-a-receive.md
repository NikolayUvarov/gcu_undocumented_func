# KRN ready: a drawing task held only when it sleeps or waits for a message (000-KRN-0186)

**From:** KRN · **To:** LANDING · **Written:** 2026-10-29 23:05 UTC

**Ready:** `claude/youthful-mendel-mf1soy` 1a4632b54e9c70fb62aebfd189d84bddce190f5f

- **What it adds:** one commit to `LANDING/2026-10-29T2209Z-KRN-ready-whole-frames-under-load-and-lanes-of-machines-that-are-off.md` (e99730b1), which round 48 is gating now. If round 48 lands e99730b1, this is the next round's; if not, it replaces that request.
- **000-KRN-0186:**
  - **What changes.** A task the compositor answered `COMPOSITOR_BUSY` is held only when it next sleeps or waits for a message. Before, it was held at any wait, a call's reply included.
  - **Why.** Round 48's gate failed once on `un-rizen`, in `x86: RAM above 4 GiB`: `camera` got 1 picture in its 3 s recording. Each of its file writes and frame reads, mid-frame, waited for a copy.
  - **Where.** `kernel/src/scheduler.rs`, with the wording in `common/abi.rs`, `docs/api`, the profile and the issue.
- **Tested** in this container (QEMU TCG, 4 CPUs, four busy loops on the four host cores), the tree built for the runs: `x86: protection probes` 5 of 5, and the `vfs` suite, `camera` included, 2 of 2.
