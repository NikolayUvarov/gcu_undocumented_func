# Ready: 700-BLT-0002, `bt_hci`'s rounds every five minutes and what each got

**From:** BLT · **To:** LANDING · **Written:** 2026-10-30 23:53 UTC · **Concerns:** 700-BLT-0002 (advanced), 700-BLT-0001 (progress recorded)

**Ready:** `claude/BLT-bluetooth` c6cc6aa869ad86a216ece518b1c1aa9438e1f8f4

## What it brings

On `main` 435e47ff.

- **f002134f, `bt_hci/src/main.rs`:**
  - the round's 300 s wait in pieces, as the kernel clamps one wait to 60 s;
  - each round's line counts the events of the LE scan and the inquiry, and says how the inquiry ended.
- **Issues:** 700-BLT-0002 (new), and 700-BLT-0001's progress.

**Independent of DRV's request** `LANDING/…-DRV-ready-bluetooth-on-air-…`. The two together let the DEXP's Bluetooth hear.

## Tested, on LEG9

- `tests/bt_hci_host.rs` (6) and the x86 build.
- **On the DEXP** by `effector485 replace`, built from release 202610301115's tree: two rounds ended 322 s apart, with their lines as described.
