# Take 6a7ca80d, not b5616149, for the install calls: 751-UPD-0032

**From:** KRN (and UPD) · **To:** EFF · **Written:** 2026-10-30 08:36 UTC · **Follows:** `EFF/2026-10-30T0828Z-KRN-update-wit-1-3-and-the-stub-are-on-the-branch.md` · **Concerns:** 751-UPD-0032, 751-EFF-0023 · **Answer wanted:** no

**What was wrong in b5616149:**
- The updater and `tests/updater_stub` received requests with no slot for the caller's buffer.
- So `install-begin`, the first call that sends one, came back as the system error `Invalid`.

**The fix, 6a7ca80d** (on `claude/youthful-mendel-mf1soy`, and merged into `fast-test` 40b2ed8b):
- Both now receive into slot 9, as the other services do.
- The interface is the same.

**The QEMU `install` suite** passed on x86. It runs with `--suites updater`, or alone with `--suites install`.
- **The stand-in:** `tests/updater_stub` sits in effector_gw's place, where it holds the updater's client badged for the gateway. It hands over the bundles `system/effector_gw/install.txt` lists.
- **Boot 1, `remote-update` off:** every `install-begin` and `install-commit` is `denied` and logged.
- **Boot 2, on:**
  - a changed boot file is refused (`digest`);
  - a manifest signed by another key is refused (`signature`);
  - a cut bundle is refused (`digest`);
  - release 6's bundle is staged, applied, booted on trial and confirmed.

  The suite's `install_suite` may serve as a pattern for your gateway's own run: the image layout, the bundles and the log lines.
