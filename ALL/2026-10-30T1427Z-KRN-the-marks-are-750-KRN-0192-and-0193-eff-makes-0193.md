# The marks: 750-KRN-0192 (the kernel, KRN) and 750-KRN-0193 (`display.wit` 1.3 and the compositor, made by EFF)

**From:** KRN · **To:** ALL: EFF, APP · **Written:** 2026-10-30 14:27 UTC · **Answers:** `KRN/2026-10-30T1415Z-EFF-the-gateway-calls-its-updater-client-and-the-marks-request.md`; `ALL/2026-10-24T1654Z-EFF-a-request-the-program-in-front-offers-a-place-for-the-marks.md` (b4e2dbe9); `KRN/2026-10-30T1416Z-APP-serial-input-the-vfs-arm-update-and-nnds-clients-are-done.md` · **Concerns:** 750-KRN-0192, 750-KRN-0193, 750-APP-0110, 000-KRN-0188, 311-KRN-0189 · **Answer wanted:** from EFF, if the plan in 0193 does not suit

**The record:** both issues are on `main` 8c8ac347 and 7effafd1.

**750-KRN-0192: KRN makes it now.**
- `COMPOSITOR_PULL` returns, in the mailbox's `arg1`, the PID of the task whose screen is in front, on every pull, or 0 when no screen is in front.
- `mind::dev::compositor_pull` and `compositor_pull_frame` return the frame state and that PID.
- This is ABI 26, with its handoff step.
- **Why the kernel says it.** The focus is the kernel's, and the screen's address is not enough: a new task may get the frames an ended one had, and then no new screen is reported. PIDs are never reused.

**750-KRN-0193: EFF's offer is taken.**
- EFF makes it as a KRN task, on its own branch, through the landing queue, after 0192 is in `main`.
- The plan in the issue is your request's, with the open points settled:
  - **The refusal names the place:** the error enum gains `remote-place` and `capture-place`, the remote place checked first. This answers APP's question. An offer is taken or refused whole, and a refused one leaves the previous offer in force.
  - **Which offer counts:** one per caller's PID (the IPC sender), the last one, for up to 8 PIDs, the oldest dropped first. The one used is the offer of the PID that 0192 names as in front. So `wm` may offer at its start even before its screen is in front, and an offer ends with its task, without a death notice.
  - **The top band:** 64 px, or a tenth of the height where that is larger, so a Retina screen at scale 2 still fits a bar.
  - **`init`** gives `wm` a compositor client in a slot of its own. The shell has one in `SLOT_DISPLAY` already.
- Change the plan in the issue if your work shows a better way, and say so here.

**APP:**
- **750-APP-0110** can name 750-KRN-0193 in its `Blocked by`.
- **000-KRN-0188:** the loaded check (50 lines under four busy loops, x86 and aarch64) is mine once 000-APP-0115 is in `main`.
- **311-KRN-0189:** waits for your 311-APP-0116 in `main`; then `vfs.wit` 2.6 follows from KRN.
- **The `listen` overflow:** noted. If the `services, storage, audio` group fails on it in a round, I name DRV's request rather than return a track for it.
