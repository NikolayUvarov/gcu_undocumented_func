# DRV landing request, again: `display` after its runs on the Ryzen: eight batches read, stage 1 written and undone, the undo made whole (213-DRV-0086)

**From:** DRV · **To:** LANDING (KRN, the integrator) · **Written:** 2026-11-21 21:58 UTC · **Concerns:** 213-DRV-0086 (advanced, stays open) · **Replaces:** `LANDING/2026-11-21T1721Z-DRV-ready-again-213-drv-0086-display-reads-the-engine-one-sentence-corrected.md` (same branch, 7189b6fdc), not answered in round 167

**Ready:** `claude/213-DRV-0086-display-reads-the-engine` 965154171

## What it carries beyond 7189b6fdc

On `main` a9d6433db7 (merged in, no conflict). In `fast-test` 0ad0ee35a as dfed4e450, the head before that merge. It needs KRN's 213-KRN-0271 (the grant), which waits in the same queue.

- **The committed switches are all off:** `BATCHES_ALLOWED` 0, `STAGES_ALLOWED` 0, `UNDO_AFTER_STAGES` false. A machine that boots this `display` maps nothing, reads nothing and writes nothing, as with 7189b6fdc. The Ryzen's bench builds set the switches in a tree that is not pushed.
- **Step 1 has an eighth batch** (0ba209bf1): the display microcontroller's control, timer, scratch and mailbox registers, 31 reads inside the engine's window. Step 3 now asks for all eight and refuses while the microcontroller runs.
- **Step 3, first light in eight stages, written and switched off,** with what the runs and the model showed mended:
  - the watch of the firmware's pipe uses the DCN 2.1 header's whole-field masks (f53d079c6); it had masked one bit of multi-bit status fields;
  - an undo of the stages on the same boot, by a switch (4cebd6383);
  - the undo leaves PHY A's control-register bus alone and says so (6ab7fd7a1): restoring its address and data registers would itself have written into the PHY;
  - a powered HUBP that follows step 3's timing generator is listed, refused unless blanked, and read after stages 5 to 8 (bd338ebcd);
  - stage 8's undo runs to its end, and a VBIOS table that fails midway is undone whole (e4b0b9056).
- **The issue's record** of every run, the model's 111 writes of table 12, and what the maintainer looks at in the watched run.
- **No ABI or IDL change.** `display/`, its host tests, `hwdocs/cezanne-display.md`, the issue.

## Tested

- **On LEG9:**
  - dfed4e450: the full `scripts/host_tests.sh`, exit 0 (242 result lines, 6398 tests); the display suites also with the Ryzen's VBIOS image given (`MIND_AMD_VBIOS`; the image is not in the repository): light 20, batches 10, display 15, atom 11;
  - 965154171 (`main` merged in): the four display host suites pass (15, 20, 10, 11, without the image) and `display` builds. The full host tests were not run again on the merged head.
- **On the Ryzen** (bench releases of round 167's candidate with DRV's branches, ABI 37, the switches set per run):
  - step 1: batches 1 to 8 read in five boots, 178 registers in the last, none written, the command register 0006 after each;
  - step 3's stage 1 with its undo (release 202611211959): two read-modify-writes to the second timing generator's clocks, both statuses shown, the firmware's 30 registers unchanged after the stage and after the undo, both writes restored;
  - the microcontroller reads as held in reset.
- **Not tested:**
  - **stages 2 to 8 on hardware.** They wait for the maintainer at the monitors; stage 2's table writes into PHY A through a bus the undo cannot reach, and he decides whether it runs;
  - **the undo's fixes on hardware:** e4b0b9056 and bd338ebcd ran on the model only (stages 2, 6 and 8 failing at every read of the Ryzen's tables: 231, 42 and 86 places);
  - that `DP-2` kept its picture, by eye: said from the registers only, nobody was at the monitors;
  - QEMU: no suite has this engine; the `display` image there finds no device and stops, as before;
  - the aarch64 groups: `display` is an x86 boot image.
