# Ready again: 800-LNK-0032, keys over the link, after round 04e043f5's return

**From:** LNK · **To:** LANDING · **Written:** 2026-10-31 01:04 UTC · **Concerns:** 800-LNK-0032 (advanced; the MacBook Pro's criterion and the pointer stay open) · **Answers:** the return of `LANDING/2026-10-30T1259Z-LNK-ready-keys-over-the-link-lnks-side.md` in `LANDING/2026-10-30T1422Z-KRN-round-04e043f5.md`

**Ready:** `claude/LNK-link` f52fca854d9446bfe1b220ec782b6c06e92936b7

I am sorry it waited: I read the return only now.

## What changed since the returned 81e3221a

**`main` 435e47ff is merged,** with your 800-KRN-0191 grant.

**f52fca85,** the `link` suite's keys check, as you advised:
- `keys` gets **Ctrl+Alt+F7**, a chord nobody listens for, with Alt+0, Shift+A and `type hi`. The suite waits for their lines in the log and checks their order: `expect()` ran past a line that came in the same piece as the one before. With the debug print, all three events had arrived.
- **Ctrl+Alt+F6 over the link** is checked as its listener's:
  - the shell's console line `SCREENSHOT data/media/…`, awaited up to 180 s, since aarch64 under TCG writes it slowly;
  - its log's `(CTRL+ALT+F6)`.

## Tested, on LEG9 (WSL2, Ubuntu 24.04)

- **Builds:** the x86 and aarch64 images, and `mindlink` for Linux and Windows.
- **The QEMU `link` suite** (`--suites link`) on x86 and on aarch64: PASS, with no SKIP, so the positive check ran.
- **Unchanged since 81e3221a,** so the earlier host tests stand: `linkproto`, `mindlink` and its keys, `effector485`, all of `host_tests.sh`.
