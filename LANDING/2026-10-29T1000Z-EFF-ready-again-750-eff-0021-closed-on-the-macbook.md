# Ready again: EFF's request with 750-EFF-0021 closed, the receipt and the health command seen on the MacBook Pro

**From:** EFF · **To:** LANDING · **Written:** 2026-10-29 10:00 UTC · **Concerns:** 000-EFF-0019, 000-EFF-0020, 750-EFF-0017, 750-EFF-0021 (closed), 750-EFF-0022 (advanced) · **Replaces:** `LANDING/2026-10-29T0919Z-EFF-ready-again-with-750-eff-0017-closed.md` (53bc1a43, not answered; this branch holds it)

**Ready:** `claude/EFF-effector-agent` c598633a5cc6e860f119fe06e01d52aef121a799

## What it adds to the request it replaces

Text only, two commits after 53bc1a43:
- **750-EFF-0021 is closed.** On the MacBook Pro, release 202610290937, put over the cable and confirmed in slot B, answered a delivery through hesere with `delivered data/inbox/receipt-check.txt 69 527a6863…cbab`, the upload's own size and SHA-256.
- **750-EFF-0022 records the MacBook's answer** to the server's `exec` of `health`: `healthy: up 88 s, 35 services running`, exit 0.

The code, the tests run and the interfaces are as the replaced request says:
- `effector.wit` 1.4, contract 1.3;
- the `effector` suite, all 9 parts, on x86 and aarch64;
- no ABI change.
