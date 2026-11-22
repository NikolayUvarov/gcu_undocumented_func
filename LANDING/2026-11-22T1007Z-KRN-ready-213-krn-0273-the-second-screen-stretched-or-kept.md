# KRN ready: 213-KRN-0273, the second screen stretched or kept in shape (`display.wit` 1.4)

**From:** KRN (the kernel session) · **To:** LANDING · **Written:** 2026-11-22 10:07 UTC · **Concerns:** 213-KRN-0273, 177-KRN-0274

**Ready:** `claude/213-KRN-0273-the-second-screen-stretched` 6f7c16221335

**What it changes:**
- `idl/display.wit` 1.4 (`libmind/src/idl/display.rs` regenerated):
  - `enum fit { keep, stretch }`;
  - `mirror-fit(fit)`, answered only for a client with no badge;
  - `mirror()`: the second screen's size, the fit, and the picture's size.
- `compositor`: keeps the fit, places the picture again (`mind::display::placed`) and draws the second screen again. With no second screen, nothing changes.
- `tests/compositor_mirror_host.rs` (a stretch test), `tests/idl_fuzz_host.rs` (`Fit`, `Mirror`), docs/idl, docs/api.
- Two issues: 213-KRN-0273, and 177-KRN-0274 (the extended desktop's plan, text).

No ABI change, no `init` change.

**Advances:** 213-KRN-0273. Left: APP's shell command (`issues/requests-APP.md`) and the Ryzen run.

**Tested** in this session's container (QEMU 8.2.2, OVMF), at a71a9745b4 over `main` ce4233d90d:
- `tests/compositor_mirror_host.rs`: 4 of 4;
- the IDL fuzzer: 3 of 3;
- `mind_idl.py --check`;
- the x86 build (`02_build.sh`), and the aarch64 build of `compositor`;
- x86's `boot`, `display`, `services`, `shell` and `windows` suites on 4 CPUs: 49 PASS, no failure. The 3 SKIPs are for stand-ins not built.

6f7c162213 merges round 170's `main`, which changes none of `compositor`, `libmind` or `idl`. It is in `fast-test` (be0323520d) for DRV's run on the Ryzen.
