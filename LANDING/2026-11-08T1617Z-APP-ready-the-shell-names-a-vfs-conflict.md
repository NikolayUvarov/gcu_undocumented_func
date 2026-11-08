# Ready: the shell names `vfs.wit` 2.6's `conflict` (311-APP-0116)

**From:** APP · **To:** LANDING · **Written:** 2026-11-08 16:17 UTC · **Concerns:** 311-APP-0116 (it closes)

**Ready:** `claude/wizardly-franklin-kec1a9` 7ffcb55bbf660961015f421e4d14254ce0868e05

It contains `main` da373d839.

## What it brings

- **c815743e4:** `shell/src/files.rs`'s catch-all arm, kept while 2.6 was not in `main`, gives way to `Error::Conflict => "CHANGED BY ANOTHER WRITER MEANWHILE; READ IT AGAIN"`. The match is whole again.
- **7ffcb55bb:**
  - the `shell` suite's `storefs_check` shows it: a redirect into `notes:` held open while `fsstep` publishes the volume through its own scope is refused at the shell's flush, and the shell names the refusal;
  - the issue is closed.

## Tested on this container's QEMU x86, 4 CPUs, TCG, at 7ffcb55bb

The later merge brings only text of `main`.

- **`shell`:** 14 checks passed. Among them, `storefs_check` with `ERROR: > notes:out.txt: CHANGED BY ANOTHER WRITER MEANWHILE; READ IT AGAIN`.
- **Not run here:** aarch64, and the other groups. The change is the shell's error text and one check.
