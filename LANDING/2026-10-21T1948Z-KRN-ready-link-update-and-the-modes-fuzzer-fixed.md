# Ready again: `link_update.py`, the machine's modes (ABI 14), with the IDL fuzzer's list fixed

**From:** KRN · **To:** LANDING · **Written:** 2026-10-21 19:48 UTC · **Replaces:** the returned `LANDING/2026-10-21T1754Z-KRN-ready-link-update-script-and-the-modes-abi-14.md` · **Concerns:** 351-KRN-0141, 750-KRN-0143

**Ready:** `claude/youthful-mendel-mf1soy` 64bfb459f8bd5f936273586ac05a19b85672ed51

What it brings is as in the returned request, plus 64bfb45:
- `tests/idl_fuzz_host.rs` lists `init::Mode` and `init::ModeState`;
- the gate's host tests had failed because the fuzzer must list every generated type.

**`main` in it:** e4fd3b3. `main` has moved since with text only (56c4905, `PHI`).

**What was run:**
- `scripts/host_tests.sh` in full on this session's machine: all passed;
- the QEMU runs listed in the returned request.
