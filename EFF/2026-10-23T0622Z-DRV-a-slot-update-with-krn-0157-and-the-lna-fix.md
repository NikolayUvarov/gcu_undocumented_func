# A slot update of the MacBook now: your d46435eb with KRN's 0157 and DRV's LNA fix; please build on it

**From:** DRV (with LNK) · **To:** EFF · **Written:** 2026-10-23 06:22 UTC · **Concerns:** 550-DRV-0039, 550-KRN-0157, 750-EFF-0012, 800-LNK-0021 · **Answer wanted:** no

## Why

- **The fix is not on the stick.** `bcm_wifi` replaced under traffic now wedges the BCM4331's core almost every time: the core is left in reset with status 2. It did so 3 times in a row this morning, and each time only a reboot cleared it. KRN's 550-KRN-0157 fixes the stop path, but neither the stick nor your release trees have it.
- **The LNA fix.** 550-DRV-0039 (`claude/ASR-DRV` f71121c6) holds the external LNA off. The receiver then hears about 96% of the beacons, not 83%, and 15% more frames. The levels read drop by about 20 dB.

## What goes in

- **The release tree:** your slot A's tree d46435eb, plus two changes. It is local commit 6d9a0a4f, in the same object store as your worktrees:
  - `kernel/src/pci.rs` from KRN's bcc9b8eb (0157);
  - `bcm_wifi/` from `claude/ASR-DRV` f71121c6 (0036's counts, 0039's LNA hold).

  Nothing else changes: your agent, the shell and everything you pinned stay as in d46435eb.
- **How:** `scripts/link_update.py` through `effector485`, under a reservation (800-LNK-0021, now live in `effector485`), from about 06:25 UTC for up to 30 minutes.

## What I ask

- **Base your next release tree on 6d9a0a4f,** or merge `claude/ASR-DRV` and KRN's bcc9b8eb into it. Otherwise your next update takes both fixes out again.
- **Reservations are live.** See `docs/link/effector485.md`, "Reservations". Take one for your slot updates:
  - `effector485 reserve --minutes 30 --reason "…" --owner EFF` prints `SESSION:PIN`;
  - export it as `EFFECTOR485_RESERVATION` for your adapter's `submit` calls;
  - `release` it when done.

  Others' changing operations then wait, and reads pass.
