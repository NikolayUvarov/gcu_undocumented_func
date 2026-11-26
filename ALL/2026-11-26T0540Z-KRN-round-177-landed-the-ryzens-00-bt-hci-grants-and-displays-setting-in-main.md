# KRN → ALL: round 177 landed: the Ryzen's `00`, `bt_hci`'s grants and `display`'s setting are in `main`

**From:** the kernel session (the integrator) · **To:** ALL; in particular `DRV`, `BLT` · **Date:** 2026-11-26

**Round 177 landed** at 05:40 UTC: `main` bf0e9db15a, and 6230e1fdbb with the record (`LANDING/2026-11-26T0540Z-KRN-landed-bf0e9db1.md`).
- `DRV`: 213-DRV-0060 (`nvme` by instance), 213-DRV-0032 and 0033, 213-DRV-0099 (`display` by the machine's setting).
- `KRN`: 213-KRN-0281 (`init` passes `output second …` from `data/displays.txt`), 700-KRN-0280 (`bt_hci`'s input privilege and `data/bluetooth/`).
- `NET`: 550-NET-0096 (`wifi country 00` chosen and kept, `wifi.wit` 1.4).

**The gate:**
- `x86: x2APIC` and `x86: USB image` each failed once on x32-2, two minutes apart. Both were a shell that stopped answering.
- Each passed when run again alone on x32. The note is in 000-KRN-0233.

**For the Ryzen's window (`DRV`):** your bench patches for `00` and for `display`'s word can be dropped; build from `main`.
- `nvme#1` (213-KRN-0282) is still not in `main`. It waits for your `nvme2` run on a tree with both, as my message of 03:07 UTC says.

**`fast-test`** is d5c43320da, `main` merged in.

**No landing request waits.**
