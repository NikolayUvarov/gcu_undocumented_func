# Ready: the shell's clipboard copy may pass a buffer, and the `tools` suite's clipboard check

**Ready:** `claude/wizardly-franklin-kec1a9` 208d3ea00aac9107b6a639a30589f91f54a4d2a8

It contains `main` 20fba2c5e1, and every commit of `LANDING/2026-11-13T0246Z-APP-ready-clipboard-wit-1-1-program-client-and-host-tests-by-name.md` (28bc3d93), which is in the candidate now: this one is for the round after it, and does not replace it.

- **Advances 000-APP-0032:**
  - the shell's own manager copy of the clipboard gets `CAP_GRANT`: get-part and set-part lend a buffer, so without it the shell's Ctrl+V and Ctrl+Insert "did not answer" (e18d2811);
  - the `tools` suite's `clipboard_check`, skipped until `clipboard` is a boot service (000-KRN-0235): `edit` copies a line through the service, the shell's line and a second `edit` paste it, the service logs lengths alone.
- **Tested:**
  - on a local merge with KRN's `claude/000-KRN-0235-clipboard` 7828fbd6 (never pushed; x86, 4 CPUs): `tools` (11, the clipboard check passing with the fix, failing without it) and `wm` (17, `wm`'s programs `WITH … clipboard`);
  - on this branch (x86, 4 CPUs): `tools` (10, the clipboard check skipped) on 208d3ea0, the same code as 208d3ea0; host tests as in 0246Z.
- **Merges** with every request waiting: cleanly with KRN's 0235, NET's sshd and 0073, UPD's root keys; with STO's `relaxed-meitner` only in `issues/312-APP-0134-…md`'s metadata line, as 0246Z said (text only, keep both).
