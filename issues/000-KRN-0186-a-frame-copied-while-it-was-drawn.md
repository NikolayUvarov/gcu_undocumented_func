# 000-KRN-0186 — The compositor copied a frame its program was drawing

**Type:** kernel track (`compositor`, the isolation suite's check) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** — · **Constitution:** MC-12.2

## Problem

- **What the check asks** ([000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done)): `app2` numbers its frames in their colour and draws the bottom half 3 ms before the top. A screen whose bottom half is newer than its top shows a frame copied while it was drawn.
- **It failed twice on 2026-10-29:**
  - in round 45's gate, on `un-rizen`'s second lane while the first ran other groups: top 238, bottom 241;
  - in this session's container, with the guest paused for each screen (QMP `stop`, `screendump`, `cont`): top 9, bottom 12.
- **What the second run shows.** A paused screen is what the video memory holds, so the newer bottom was copied there. QEMU's reading of a changing screen did not cause it.
- **Rarely seen.** The same group passed when run again alone on `un-rizen` (34 s), and in every earlier gate.

## Plan

- Find when the compositor copies a program's surface against when the program has finished a frame: the order of `present`, its reply and the copy, and whether a program can draw into the surface while it is copied.
- Find why a slow or loaded machine shows it, and a fast one does not.
- Fix the protocol or the compositor, not the check.

## Cause (2026-10-29)

- **The hold ran out mid-copy.** The kernel holds the task for the copy until `COMPOSITOR_RELEASE` or 50 ms (000-KRN-0086). The compositor compared and wrote each pixel to the framebuffer from the top, under the hold.
- **Framebuffer writes are slow where the check failed.** Under TCG, writes to QEMU's video memory are far slower than to memory. In the paused run one copy spanned some 50 screens with its top done and its bottom old: `(9, 6)` many times, then `(9, 12)`.
- **What followed.** Once the 50 ms passed, `app2` ran on and drew the next frames. The copy, still on its way down, took their bottom half. All task selection goes through `ready()`, which honours the hold, so no other path let the task run.
- **Two more ways to a copy of a frame being drawn,** found by reading the code, not seen in a run:
  - **The copy as drawn.** The compositor asks every millisecond or so while it is answered `COMPOSITOR_BUSY`, and `app2` waits only 2 ms of each frame. On a loaded machine the asks could miss those waits for `FORCE_MS` (250 ms), and the screen was then copied as it was drawn, with no hold.
  - **A bound in the guest's time.** The guest's time runs on while the host does not run the compositor's virtual CPU, so a time bound alone can end a hold in the middle of the copy.

## Progress

- **The compositor** (`compositor/src/main.rs`):
  - under the hold it only reads the screen into its shadow buffer and notes each row's changed columns, then releases;
  - it writes the framebuffer from the shadow after the release;
  - its watch line names the longest hold of each second.
- **The kernel** (`kernel/src/scheduler.rs`):
  - a task answered `COMPOSITOR_BUSY` is held as it next starts to sleep or to wait for a message, where its frame ends, and the compositor's next ask finds it so;
  - it is not held so at a call's reply. Round 48's gate (e99730b1) held it at any wait, and there `camera` got 1 picture in 30 frames of its 3 s recording, once, on `un-rizen` (`x86: RAM above 4 GiB`). Each of its file writes and frame reads waited for a copy. That did not happen again in this container. With the narrower rule, under four busy loops, `x86: protection probes` passed 5 runs of 5 and the `vfs` suite 2 of 2, `camera` included;
  - a hold with nothing to copy is released at once, and a change of focus releases it;
  - a hold also ends when the compositor waits or ends, which it does only after its copy;
  - the time bound is 1 s (`HOLD_NS`), against a compositor that neither releases nor waits, and the kernel logs a hold it ends so.
- **The compositor** releases after any dirty copy, including `FORCE_MS`'s.
- **The check:** each screen is taken with the guest paused (QMP `stop` and `cont`). QEMU otherwise reads the screen while the guest writes it, so a copy that overtakes the read also shows the bottom newer.
- **The docs:** `common/abi.rs`, `docs/api/README.md` and the profile's row on task screens.
- **Runs in this container** (QEMU TCG, 4 guest CPUs, the `x86: protection probes` group, four busy loops on the four host cores):
  - **A mistake corrected.** `scripts/ci_local.sh` with a single test group builds nothing. Every run of that group here before the counted runs below used images built before this task. So the earlier notes in this file that runs failed "with only the first fix" or "with the hold at the next wait too" were wrong: those runs tested the old code.
  - **The old code** (before this task): 7 of 15 runs saw a frame copied while it was drawn (top 92, bottom 101; top 13, bottom 18; …).
  - **The code of `d26de9fd`**, built for the runs: 18 runs in a row passed.
    - 8 of them carried temporary counters, sent to QEMU's debug port. Over each whole run (the boot and the suite) they counted 375 to 441 busy answers and 750 to 830 holds, 162 to 181 of them made at a task's next wait. Per run, 0 to 6 holds ended because the compositor waited before its next ask. None ended by the time bound, none of the copies was made without a hold, and the held task was never found running.
    - 10 had no counters.
- **The acceptance runs** (2026-10-29, `claude/youthful-mendel-mf1soy` 46efb7b5): `un-rizen`, both of the track's lanes at once.
  - Each lane built the tree, then ran `x86: protection probes` ten times.
  - All 20 runs passed. Each run took 423 to 436 screens, the guest paused for each, and frames reached 169 to 191.
- **Left:** landing; then the issue closes.

## Acceptance criteria

- The check passes 20 times in a row under load: two lanes on `un-rizen`, and the guest paused for each screen.
- The cause and the fix recorded here.

## Related

[000-KRN-0086](../issues-done/000-KRN-0086-the-compositor-copies-whole-frames.done); round 45's gate (LANDING/ on `internal-dev-messages`).
