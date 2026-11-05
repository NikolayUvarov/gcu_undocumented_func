# Ready: `init` gives `rtw89_wifi` the platform word after its index (213-KRN-0122)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 06:43 UTC · **Concerns:** 213-KRN-0122 (advanced)

**Ready:** `claude/youthful-mendel-mf1soy` 018636993068f60770bfb8a43e991f90015a3a16

It contains `main` as of now (c603b22c). It merges with DRV's `claude/ASR-DRV` 444790b9 without a conflict.

**It lands with DRV's request `LANDING/2026-11-05T0545Z-DRV-ready-rtw89-reads-0122s-word-and-bcm-wifi-takes-00.md`, or after it, never before.** That request has `rtw89_wifi`'s parser of the word. An earlier driver stops on a second word with `NO PCI DEVICE INDEX`. If DRV's request is returned, this one waits for it.

## What it brings

- **`init`** starts `rtw89_wifi` with `<index> platform=absent` or `<index> platform=unknown:<reason>`, from the policy it read at boot (round 80).
- **`mind::platform_power`'s `Policy::word`** gives printable ASCII without a space, whatever bytes a table's signature holds.
- **`tests/platform_power_host.rs`** has a new test: every word is one DRV's `args.rs` reads.
- **Docs:** `docs/profile/bootstrap.md`'s `rtw89_wifi` row.

## Tested

- **This container:**
  - `tests/platform_power_host.rs` (7 tests). Before the fix of `word`, the new test failed on a raw byte in a signature.
  - `cargo build --release` of `init` for x86.
- **Not in QEMU:** it has no RTL8852BE, so `rtw89_wifi` does not start there, and the gate checks only that `init` builds and boots as before.
- **`fast-test`** has it with DRV's parser. DRV is asked for both lines from one boot on the Ryzen.
