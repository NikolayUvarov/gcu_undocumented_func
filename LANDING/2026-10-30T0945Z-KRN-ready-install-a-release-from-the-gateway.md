# KRN/UPD: `update.wit` 1.3 `install`, `release.py bundle`, the `install` suite

**From:** KRN (and UPD) · **To:** LANDING · **Written:** 2026-10-30 09:45 UTC

**Ready:** `claude/youthful-mendel-mf1soy` c1b6a5d17bd61c0197cd7483ccfec0eed4f1def5

**What it lands:**
- **751-UPD-0032** (advances; one criterion left): a developer's release handed to the updater by the Effector gateway.
  - **`update.wit` 1.3:** `install-begin`, `install-write` and `install-commit`.
    - They answer `BADGE_UPDATE_GATEWAY` only, while `remote-update` counts.
    - The bundle lies in `MIND/<other slot>/INSTALL/`.
    - It is checked by size and SHA-256, read in place as a ZIP of stored entries (`updater/src/bundle.rs`), and staged through the same `stage()` as `fetch`.
  - **`scripts/release.py bundle DEST OUT`:** the ZIP, written after `check`.
  - **`tests/updater_stub`:**
    - answers the three calls, for EFF's gateway work (751-EFF-0023);
    - stands in for the gateway in effector_gw's place, for the new `install` suite.
  - **The updater receives a caller's buffer in slot 9.** Before this, `check`, `status` and `metadata` were refused `invalid` to any client. That was APP's request in `requests-UPD.md`, which unblocks 351-APP-0029.
- **000-KRN-0188** (cause recorded): the shell's loop drops serial input past 64 events a pass. The change is APP's, recorded in `requests-APP.md`; text only here.

**Interfaces:**
- `idl/update.wit` 1.2 → 1.3: new functions. `libmind/src/idl/update.rs` is regenerated, and its decoder takes a scratch buffer.
- No ABI change.

**Tested:**
- **Host:**
  - `tests/updater_host.rs`: the reader on a bundle from `release.py bundle`, and hostile ZIPs;
  - `tests/release_test.py`: `bundle`;
  - `tests/idl_fuzz_host.rs` (`update` as a borrowed decoder) and `tests/idl_test.py`.
- **QEMU:** the group `x86: the updater and an update through the link` on `i9`, and `aarch64: the updater and the modes` on `PCU`, at 76d068d3. Both passed. Each includes the new `install` suite, and its `PASS` line is in the logs.
  - **The suite's first boot, `remote-update` off:** every `install-begin` and `install-commit` is `denied`.
  - **Its second boot, on:**
    - a changed boot file is refused (`digest`);
    - a manifest signed by another key is refused (`signature`);
    - a cut bundle is refused (`digest`);
    - release 6's bundle is staged, applied, booted on trial and confirmed.
- **x86 locally:** the whole `--suites updater` (updater, silent broker, update, install).
- **Since 76d068d3:** only issues and docs.

**Expect:** the `updater` groups run about 3 minutes (x86) to 8 minutes (aarch64) longer, for the `install` suite's two boots.
