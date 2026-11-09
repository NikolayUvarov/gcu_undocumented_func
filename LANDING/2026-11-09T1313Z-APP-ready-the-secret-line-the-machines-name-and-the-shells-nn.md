# Ready: the secret line (000-APP-0126), the machine's name closed (000-APP-0123), and the shell's `nn` (262-APP-0112 closed)

**From:** APP · **To:** LANDING · **Written:** 2026-11-09 13:13 UTC · **Concerns:** 000-APP-0126 (it advances; `get -u` is NET's), 000-APP-0123 and 262-APP-0112 (both close), 000-APP-0120's follow-up · **Replaces:** `2026-11-09T1144Z-APP-ready-a-console-programs-secret-line.md`, not answered yet

**Ready:** `claude/wizardly-franklin-kec1a9` ddc596896e41ae326b2bbc12273de796fbb6c55b

It contains `main` 39351fa0f.

## What it brings

**What the request it replaces brought, unchanged** (d1321a04d to 64a331295):
- **`shell.wit` 1.12 `secret-line`:**
  - the shell holds the call while the line is typed with no echo, then wipes its copy;
  - the line is lent for `REQUEST_SECRET_LINE` as a copy badged `BADGE_SHELL_SECRET`, through which every other call is refused;
  - `tests/secret_stub` and the `shell` suite's `secret_line_check`.
- **The broker's answer to a flow question** gets 5 s.
- **ca986c3e3** is 000-KRN-0224's lines, the same as `main`'s.

**New:**
- **015eaec46, 69dc007f2:** the `link` suite sets `hostname` from the link's client at once, and 000-APP-0123 is closed.
- **adcf40d71, 7ccbe1338, 7595ce953:** the shell's `nn load <path> <id>`, `nn unload <id>`, `nn run <id> <file of rows>` and `nn bench <id>`, on its own client of `nnd` (docs/nn/runtime.md section 7, now 1.2).
  - In a script they need `requires: nn`.
  - The rows' text, the ID and the output text are `shell/src/nnrows.rs`, host-tested.
  - The `nnd` suite's `shell_nn_check` covers them.
  - docs/tools, in both languages.
- **ddc596896:** 262-APP-0112 is closed, with what was not run (KVM, a real machine, `models:`) left to the main task 262.

## Tested on this container's QEMU x86, 4 CPUs, TCG

- **At c2bea2cca with `main` c92256a3, `shell` and `net` in one run:** 52 checks passed, as in the request it replaces.
- **At 015eaec46 with `main` 39351fa0f, `link`:** passed. `hostname DEXP-LINK` over the link was set at once and read back on the console, and `my_pc` was refused.
- **At 7595ce953 (the image built at adcf40d71), `nnd` and `shell` in one run:** 19 checks passed.
  - **`shell_nn_check`:**
    - NRL's pinned input from a text file came out as the reference outputs within NRL's tolerance, charged 3 rows and 1893 work;
    - a wrong-width file was refused with its line;
    - `bench` gave 1602 rows a second, for that configuration alone;
    - a script with `requires: files` was refused at `nn run`, and one with `requires: nn` ran;
    - after `unload`, a run was refused.
  - **`secret_line_check`** passed again.
- **Host:** `tests/shell_host.rs`, 31 tests. `scripts/mind_idl.py --check` is clean.
- **Not run here:**
  - aarch64, where the secret check skips without a stub build;
  - the other groups.
