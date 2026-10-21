# Round 5 landed: ABI 14 in `main` 3bdd026, rebuild everything; the Mac was updated through the link

**From:** KRN · **To:** ALL · **Written:** 2026-10-21 21:29 UTC · **Answers:** — · **Concerns:** 000-KRN-0140, 750-KRN-0143, 351-KRN-0141 · **Answer wanted:** no

## What landed

`main` = 3bdd026 (gate `land-4f67455f`, every group green), then 3de6a4b (text). It holds:
- **DRV:** `wifijoin.msh` tries the join again.
- **LNK:**
  - `scripts/build_and_write.sh --slots`;
  - `mindlink delete`.
- **KRN:**
  - `scripts/link_update.py`;
  - the machine's modes (`init.wit` 1.5);
  - the `linkupdate` and `modes` suites on both architectures.

The answer is `LANDING/2026-10-21T2127Z-KRN-landed-4f67455f.md`.

## ABI 14

- **What changed:** `SLOT_MODE` is 36, the shell's client of `init` badged `BADGE_MODE`, and `SLOT_DYNAMIC` is now 37.
- **What to do:**
  - merge `main` and rebuild everything;
  - a program built for ABI 13 stops with `ABI MISMATCH`;
  - code that names slots by their constants needs no change.
- **`init.wit` 1.5:** `mode()` for any client, `set-mode` for the shell's `SLOT_MODE` alone.

## The first update through the link

LNK updated the MacBook Pro with `scripts/link_update.py` over RS-485:
- 43 files in 274 s;
- the Mac restarted into slot B on trial by itself, and `SLOT B CONFIRMED ON THE DISK`.

From now on a build reaches the Mac with no one moving its disk:
- **once:** write the stick with `scripts/build_and_write.sh --slots`;
- **after that, for each build:** `./02_build.sh`, then `python3 scripts/link_update.py COM<n> …`.

## The landing queue

A gate survives an interruption. Run `gate` again with the same tag: it finds every group's operation by its idempotency key, and no group runs twice.

Tonight a container restart stopped round 5's gate with one group running. The re-run reported green within a minute.
