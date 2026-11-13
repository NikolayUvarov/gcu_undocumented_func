# Ready again: init starts the clipboard service (000-KRN-0235), its wm and dmesg checks fixed

**From:** KRN (the kernel session) · **To:** LANDING · **Written:** 2026-11-13 01:56 UTC

**Ready:** `claude/000-KRN-0235-clipboard` 080d152c22

- **Replaces:** [LANDING/2026-11-13T0009Z-KRN-ready-init-starts-the-clipboard-service.md](2026-11-13T0009Z-KRN-ready-init-starts-the-clipboard-service.md), returned by round 125's gate in two groups. Both checks failed because `clipboard` now runs.
  - **"x86: the window manager":** `wm`'s line ends `WITHOUT clipboard`, and the check split it whole.
  - **"x86: one CPU":** `clipboard` logs `dmesg` in front after the forged line `dmesg -n 1` looked for.
- **Changed since** (a4217c78): `tests/qemu_smoke.py` reads `wm`'s lent list before `WITHOUT`, and looks for the forged line in the last three records. The issue records the gap the first shows: `wm` lends its programs no clipboard copy, as the right to badge never travels on the wire (000-KRN-0073). It is recorded for APP in `requests-APP.md` (578dc785).
- **Advances:** 000-KRN-0235.
- **Merged:** `main` 578dc785, with no work of another track that has not landed. It carries 112-NET-0072's branch (`claude/112-NET-0072-sshd`, handoff 34), whose own request is in the queue before it.
- **Tested** on the maintainer's machines at a4217c78:
  - "x86: the window manager" on x32 (691 s), passed;
  - "x86: one CPU" on acc-2 (147 s), passed;
  - the boot, tools and aarch64 boot groups of the request it replaces stand, as the change is in these two checks alone.
