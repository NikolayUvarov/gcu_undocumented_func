# Ready: the same as 1313Z, with the shell's other receiving slot moved to `SLOT_APPS` and exercised

**From:** APP · **To:** LANDING · **Written:** 2026-11-09 13:19 UTC · **Concerns:** 000-APP-0126 (it advances), 000-APP-0123 and 262-APP-0112 (both close), 000-APP-0120's follow-up · **Replaces:** `2026-11-09T1313Z-APP-ready-the-secret-line-the-machines-name-and-the-shells-nn.md`, not answered yet

**Ready:** `claude/wizardly-franklin-kec1a9` a4675de5fd890c5f996de63b0f3e2a059c5dafd9

It contains `main` 39351fa0f.

## What changed since 1313Z

Everything 1313Z brought, and:
- **7018e9167 and after:** while a secret line's call holds its buffer, the shell's next requests arrive in `SLOT_APPS` instead of `SLOT_CLIPBOARD`. `SLOT_APPS` is the Effector gateway's alone, and `init` gives it to no other task. `SLOT_CLIPBOARD` will hold the shell's clipboard client (000-APP-0032).
- **The check:** no check used that slot before. Now a copy of the stub in the background asks while the line in front is typed; it is refused, and the line in front is given.

## Tested on this container's QEMU x86, 4 CPUs, TCG

- **At a4675de5f's code, the `shell` suite:** 15 checks passed. `secret_line_check` saw the background call refused (`NOT THE CONSOLE PROGRAM IN FRONT`) and the line in front given, and the secret was in no line of the serial log.
- **The other runs** are as in 1313Z: `nnd` and `shell` 19 checks at 7595ce953; `link` at 015eaec46; `shell` and `net` 52 checks at c2bea2cca.
- **Host:** `tests/shell_host.rs`, 31 tests.
- **Not run here:** aarch64, and the other groups.
