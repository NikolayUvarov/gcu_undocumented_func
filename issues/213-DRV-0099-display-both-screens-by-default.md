# 213-DRV-0099 — `display`: both screens by default

**Type:** driver (`display`) · **Owner:** `DRV` · **Priority:** P2 · **Status:** open (2026-11-22) · **Blocked by:** [213-DRV-0098](213-DRV-0098-display-both-screens-at-start-in-seconds.md); the maintainer's decision · **Main task:** [213](213-amd-ryzen-pc.md) · **Roadmap:** the drivers track · **Constitution:** MC-12.1, MC-12.3, MC-12.5

## Problem

The committed switches are all off: `BATCHES_ALLOWED` 0, `STAGES_ALLOWED` 0, `UNDO_AFTER_STAGES` false. Each run on the
Ryzen was a release built with other values on the owner session's word. A machine meant to simply come up with both
screens needs a default that does it, and the evidence that the default is safe on the machines it reaches.

## Plan

- **Where the decision lives:** the switches leave the build and become a per-machine setting, kept with the machine's
  other settings, so that a release is the same for every machine.
- **The default on** only where the evidence covers the machine: the Ryzen with its two monitors (MC-12.1). Elsewhere,
  `display` reads and logs, and writes nothing.
- **What happens on a failure** at start: the undo, and the machine comes up with its first screen alone, said once in
  the log.
- **The maintainer decides** where the default is on.

## Acceptance criteria

- [ ] The setting, read at start; host cases for on, off and a failure at each stage with the setting on.
- [ ] On the Ryzen: both screens come up at every start with no build switch changed, over several starts.

## Related

[213-DRV-0092](213-DRV-0092-display-mind-cores-picture-on-the-second-output.md), [213-DRV-0098](213-DRV-0098-display-both-screens-at-start-in-seconds.md).
