# 213-DRV-0098 — `display`: both screens at start, in seconds

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** — · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.3

## Problem

The stages run as a survey. Each is logged in full before its first write, with 1 s before its writes and 3 s after
it. Step 1 reads ten batches with pauses between them. On boots 75 and 76, step 3 began about 56 s into the boot. Boot 76's
stage 16 offered the screen at 130.6 s. For a machine in use, the second screen should come up in seconds.

## Plan

- **A production order:** the same writes and checks in the same order, without the pauses and without the per-write
  lines:
  - step 1 reads only what steps 3 and 4 need;
  - the log keeps one line a stage, and the full detail on a failure.
- **How long each part takes:**
  - the tables' run times (table 12's short path, 76, the others);
  - the waits: OTG status, frames, stage 15's watch;
  - the reads.

  Measured on the Ryzen, then summed: the target is under two minutes from start, and the measure says by how much.
- **The watch** (stage 15's 300 frames) may shorten, if the runs so far allow it: a decision with the evidence.
- **The survey order** stays, behind a switch, for bring-up.

## Progress (2026-11-22)

**The production order, behind `SURVEY_ORDER`.** The switch is committed false, so the production order is the default.
True gives the survey's order of boots 70 to 81. At 0 batches and 0 stages neither order reads or writes anything.

**Where the order lives: in the log, not in the steps.** Steps 2 to 4 run the same code in either order. Only their
`batches::Out` differs:
- **The survey's** (`Log` in main.rs) logs every line, with its 20 ms pace, and makes every pause.
- **The production one** (`batches::Held`, over the same `Log`) works so:
  - it makes no pause;
  - it holds the detail lines of the part under way (a batch, step 2, a stage) in 64 KiB on the heap;
  - it logs that detail only if the part fails, and every line after it until the next part begins;
  - when the buffer is full, the oldest lines go and are counted;
  - it logs notes at once (a part's result, a refusal, a register not mapped).
- **Before step 1:** the 5 s pause is the survey's only.
- **Step 1:** in the production order it reads of each batch only the registers steps 2 to 4 take (`batches::USED`, 77
  registers at every instance the survey reads them), in the survey's order. A batch none of whose registers are taken
  (batch 7) still counts as read. On the Ryzen's state (the model's) that is 143 of the survey's 305.

**One line a stage, in either order.** Each stage ends with the survey's line, now with its time and what it did:
```
S2 PIXEL CLOCK DONE; THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS); 0.3 MS, 1 WRITES, TABLE 12 20 US (3 STEPS, 2 WRITES), THE PHY SET UP ALREADY: THE SHORT PATH
S15 UNBLANK DONE; THE FIRMWARE'S PIPE UNCHANGED (30 REGISTERS); 5025.3 MS, 2 WRITES, 1 WAITS 42420 US, WATCHED 300 FRAMES IN 4982 MS CLEAN, LONGEST GAP 50 US AT FRAME 2
```
These are the model's lines, at 3440x1440 with synthetic tables. Each line names, as far as the stage has them:
- its writes, and its waits with their time;
- each table's run without the log's time, with its steps and writes;
- table 12's path;
- stage 9's clocks;
- whether stage 13 read the pitch back as written;
- stage 15's watch, with the longest gap between two reads and the frame it came before (the head first released, e458838ac, logged the gap without its frame);
- stage 16's offer.

**The times.** At the end, `STEP 3'S TIMES (MS)` gives:
- the whole run;
- stages 1 to 8, 9, 10, 11 to 14, 15 and 16;
- the tables, the waits, stage 9's count and stage 15's watch.

`display` then logs `THE SECOND SCREEN OFFERED n MS AFTER DISPLAY'S START, IN THE ... ORDER` with:
- the time before step 1;
- step 1;
- step 2;
- step 3 up to the offer.

Step 2's last line gives its time, each transfer from GO, and the give-back.

**On the model** (3440x1440 in HDMI, frames at 60.2 Hz, synthetic tables): the production order offered the second
screen 5.96 s after step 3 began, of which:
- stage 15's watch, 4.98 s;
- stage 9's count, 0.50 s;
- the waits of stages 5, 13 and 15, 85 ms;
- the 20 ms pace of its 19 lines, about 0.4 s.

The survey's order took over 60 s on the same model. These are the model's numbers, not the Ryzen's (MC-12.2, 12.3): its
tables, its draw and its clock are not the hardware's.

**The waits the hardware needs, each by its condition in either order.** Every status and frame wait polls its
register until the condition holds, with its own limit, and fails the stage at the limit. None waits by time alone:
- **Stage 1:** `OPTC_INPUT_CLK_ON` and `OTG_CLOCK_ON`, 1 ms each. These are Display Core's own limits, and it waits
  right after the write too (`optc1_enable_optc_clock`). The survey's runs showed them in 1 to 7 us, after a 20 ms line.
- **Stages 2, 6 and 8:** the VBIOS tables' own waits and delays (table 12's 10 us, table 76's 420 us and its status
  loops), bounded by 500 ms a table, the log's time not counted. Linux runs these tables with no pause either.
- **Stage 5:** `OTG_CURRENT_MASTER_EN_STATE` within 50 ms, then two frames of OTG1 within 200 ms. These show that the
  pixel clock of stage 2 runs before anything is put on it.
- **Stage 6 (HDMI):** the generic packets' conflict clear, within 500 us.
- **Stage 9:** 30 of the firmware's frames counted, within 2 s.
- **Stage 13:** two frames of OTG1 after its global sync.
- **Stage 15:** three frames after the unblank and the underflow check, then the 300-frame watch.
- **Step 2:**
  - the grant is read right after the request, as Display Core's `acquire_engine` does;
  - each transfer's status is polled from GO, within 500 ms.

**What the survey's pauses did that no condition covered: the `DIG_START` pulse** at the end of stage 8. Its two writes
(1, then 0) hold the front end's FIFO in reset, and no status shows when that is done.
- **In the survey:** the log line between the two writes held it about 21 ms (boot 81: 95.872 to 95.893 s).
- **Display Core:** it holds it 1 us (`udelay(1)`, dcn20_stream_encoder.c).
- **Now:** the plan holds it in a step of its own, `Step::Hold`, of 10 us (`dcn::DIG_START_HOLD_US`), in either order.
  There are no new writes. The survey's order holds it the 21 ms as before, plus the 10 us.

Everything else the pauses covered is a run of writes that Display Core makes back to back too: the stages' own writes,
the DPP's and HUBP's set-up after their clocks, the MPC, and the I2C engine's set-up.

**Host cases** (MC-12.2: a model, not the hardware):
- **`display_light_host.rs`**, 38 cases, 5 of them new. Steps 3 and 4 run on the model in both orders, the survey's
  paced on the model's clock as the hardware paces it (20 ms a line, its pauses), the production one through `Held`:
  - the same writes, offers and `screen_gone`, in the same order:
    - at 1920x1080 in DVI and in HDMI, and in the EDID's 3440x1440 in HDMI;
    - for all 16 stages, left in place and undone;
    - for builds of 1, 2, 5, 8, 9, 10, 13 and 15 stages;
  - each failure path the same way, left in place and undone:
    - stage 5's frames that never move;
    - a status that never clears;
    - HUBP1 underflowing;
    - a one-off underflow in the watch;
    - the offer refused;
    - the firmware's pipe changed;
    - stage 9 without a reference;
  - with `MIND_AMD_VBIOS`, the Ryzen's own tables, warm (the short path) and cold (the long one), in the three modes:
    the same writes;
  - the production log: 21 lines on a clean run, none of a write or a plan. A stage that fails logs its detail as the
    survey's but for the times, and none of the stages before it. The undo after it is logged in full.
  - `Held`: the latest lines kept, a line cut at 255 bytes.
- **`display_batches_host.rs`**, 15 cases, 3 of them new:
  - the production order reads the survey's reads of `USED`'s registers in the survey's order, and no others;
  - `light::assess` and `light::reference` take the same from either order's step 1;
  - each of the 162 registers it leaves unread, set to 0, all ones, 5A5A5A5A or its value with one bit changed, changes
    nothing they take;
  - each register of batches 1 to 10 it reads, missing, changes what they take.
- **`display_ddc_host.rs`**, 7 cases, 2 of them new:
  - step 2 makes the same writes in either order, for a read and for each failure, the survey's paced;
  - its production log is one line, or the survey's detail on a failure;
  - batch 11 as the production order reads it gives the same read, writes and lines;
  - each register it leaves unread changes nothing;
  - each it reads is taken.
- **`display_host.rs`:** the `DIG_START` hold in the plan.
- **On the frame:** the model's frame is a whole number of stage 9's polls (16,610 us). Stage 9's count then comes out the
  same in both orders, and with it the DLG and sync values stages 13 and 14 write. On the hardware that count is a
  measure each run (268,495 kHz at boot 71, 268,522 at boot 81), so those values can differ in their last digit between
  two boots in either order, as they did between survey boots.

**Not shown:**
- the production order on the Ryzen, and its times there;
- whether the hardware needs anything the survey's pauses gave beyond the `DIG_START` hold. The 10 us hold has not run
  on the hardware.

**A proposal, not made** (the owner session's decision): stage 15's watch is 5 s of about 6 s.
- **What the runs show:** the one underflow seen (boot 74, before e20a0244f's changes) showed within 41 frames of the
  pattern going off. Four watches since then were clean all 300 frames (boots 75, 76, 79, 81).
- **What a shorter watch would give:** one of 60 frames (1 s) would have caught boot 74's and would save 4 s.
- **What is not known:** whether an underflow can come later. The watermarks are the firmware's, and DRAM's self-refresh
  is forced off. Four runs do not show that it cannot.
- **What a shorter count would give:** 10 frames for stage 9 (0.17 s) would save 0.33 s, at a third of the count's
  precision, about 0.07 % against 0.02 %.

The code keeps 300 and 30.

**Found on the way:**
- **`assess`'s comment** said that step 3 reads and writes no register step 1 has not read on the same boot. That was
  not so even in the survey. The stages read first, by their own reads, among others:
  - `OPTC_INPUT_CLOCK_CONTROL`, `OPP_PIPE_CONTROL`, the DPG and `VTG_CONTROL`;
  - the HDMI and AFMT registers;
  - the frame counts;
  - pipe 1's HUBP, DPP and MPCC.

  The comment now says so. In the production order, step 3's own reads also include registers step 1 no longer reads:
  the watch's baseline of the firmware's pipe (batch 7's pixel rates, its sync and interrupt registers) and the mark's
  `OTG_VERTICAL_INTERRUPT2_POSITION`. All of them were read on every survey boot since 70, before any write, as the
  baseline is now.
- **A stale line:** `D THE MODE STAYS 1920x1080 AT 60 HZ IN THIS BUILD` was logged by every EDID read, boot 81's
  included, which then took 3440x1440. It is gone.
- **The profile:** docs/profile/bootstrap.md's `display` row named seven batches and eight stages, and said BAR 0 is
  not read. It now says what the code does.

## Acceptance criteria

- [x] Host cases: the production order makes the same writes as the survey's, in the same order. `display_light_host.rs`
  for steps 3 and 4 at 1920x1080 (DVI and HDMI) and in the EDID's mode, with every failure path and the Ryzen's tables;
  `display_ddc_host.rs` for step 2; `display_batches_host.rs` for what step 1 leaves the later steps.
- [ ] On the Ryzen: the second screen offered within the measured time, logged with each part's share.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md), [213-DRV-0099](213-DRV-0099-display-both-screens-by-default.md).
