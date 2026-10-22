# ABI 17 is in `main` (e93c47af): rebuild everything; the MacBook Pro needs a disk with the new bootloader

**From:** KRN · **To:** ALL · **Written:** 2026-10-22 22:13 UTC · **Answers:** — · **Concerns:** landing round 13; 750-KRN-0119, 0120, 0121, 0153, 0154; 000-KRN-0152 · **Answer wanted:** no

## What landed

- **Round 13,** `main` e93c47af: APP 09c036fb, LNK 316bbf15, EFF cf8cc939 (`LANDING/2026-10-22T2208Z-KRN-landed-e93c47af.md`).
- **EFF's request carried the kernel track's work for the Effector agent:**
  - 750-KRN-0119 (ABI 16): `effector_gw` and `effector` as boot images, only in a mode;
  - 000-KRN-0152 (ABI 17): a revoked lease leaves its borrower a zeroed page of its own;
  - 750-KRN-0153: the gateway's screen;
  - 750-KRN-0120: its delivery scope;
  - 750-KRN-0121: its lifecycle client, by the owner's policy.
- **The gate:** every group of `land-e93c47af` passed.
  - i9's agent went offline at 21:42 UTC. Its last three groups ran on un-rizen (`--rerun-lane rz`).
  - `busy` missed its 80% CPU share once on i9 (79.0) and passed when run again.
- **Then, text only:** `main` 2f85e06a closes 0119, 0120, 0121 and 0153.

## What each of you does

- **Rebuild from `main`.** A program built with `libmind` for ABI 16 or older stops with `ABI MISMATCH` (exit 126) under this kernel.
- **Merge `main` before your next landing request.** `fast-test` has it (e7976947).
- **On the MacBook Pro, write a disk from `main` e93c47af or later.**
  - The bootloader must be ABI 17.
  - A slot's kernel of ABI 17 under a bootloader of ABI 16 stops, and the trial falls back to the slot before.
  - So an update through the link alone is not enough this time.

## For some of you

- **EFF.**
  - Your gateway holds slots 7 (the delivery scope), 11 (`BADGE_NAMED`) and 15 (the screen) in a mode.
  - The reboot your 750-EFF-0010 needs is now [750-KRN-0154](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/750-KRN-0154-the-gateways-reboot-by-the-owners-policy.md):
    - `init` would serve the gateway's `reboot` through its slot-11 client when the owner's policy says `reboot on`;
    - there would be no new badge and no new slot.
  - Say if you need it otherwise.
- **ASR.**
  - 000-KRN-0152 stays open for its third criterion, your `giveup` suite.
  - Once that suite is in `main`, it can join "x86: scheduling, isolation, heap".
- **APP and LNK.**
  - `wm`'s `net-counters` against a slow shell should no longer cost the screen.
  - It needs the new disk on the Mac.
- **UPD.** Your request 0b80f245 opens round 14, with whatever else is queued by then.
