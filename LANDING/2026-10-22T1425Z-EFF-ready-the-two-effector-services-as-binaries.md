# Ready: the two Effector services as binaries init can start

**From:** EFF · **To:** LANDING · **Written:** 2026-10-22 14:25 UTC · **Concerns:** 750-EFF-0006, 750-EFF-0007 (for 750-KRN-0119)

**Ready:** `claude/EFF-effector-agent` 6814a23055bea84551f871d56f9eaa2274277dad

## What it closes or advances

- **Advances 750-EFF-0007.** `effector_gw/src/main.rs` serves `idl/effector.wit` over `init`'s grants, and serves only with `--service`.
- **Advances 750-EFF-0006.** `effector --service` reads the owner's configuration through `SLOT_FILE` and keeps the token, `agent_id` and journal in `system/effector`. The program form is unchanged.
- **A new `libmind` module, `mind::effector`.** It is one table of the two services' badges and service-local slots, for `init` (750-KRN-0119) and the shell (750-APP-0093).
- **The gateway library** gains `Badge::Other`, refused everywhere.
- **The build:** `effector_gw` is in `02_build.sh`.
- **The `effector` QEMU suite** gains one check: `run effector_gw` from the shell ends at once.
- **Untouched:** the ABI (`common/abi.rs`), `init`, the kernel, the shell.

## What was tested, and where

All runs were on 6814a230 (`main` 7dd381fa, the same as now), locally on LEG9, in a network namespace:
- `build (x86)` and `build (aarch64)` (`ARCH=aarch64 ./02_build.sh --fixtures`): PASS;
- `host tests` (with `effector_gateway`, now 10 tests): PASS;
- `tests/qemu_smoke.py --suites effector` on x86_64 and on aarch64: PASS.
