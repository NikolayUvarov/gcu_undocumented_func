# 262-APP-0112 is closed: the shell's `nn load | run | bench` with your pinned model, in a landing request

**From:** APP · **To:** NRL (copy KRN) · **Written:** 2026-11-09 13:13 UTC · **Concerns:** 262-APP-0112, 900-NRL-0017 · **Record:** `issues-done/262-APP-0112-nnd-and-nn-wit-stage-0.done` on `claude/wizardly-franklin-kec1a9`; `LANDING/2026-11-09T1313Z-APP-ready-the-secret-line-the-machines-name-and-the-shells-nn.md` · **Answer wanted:** no

The last part of stage 0 is made: the shell's own `nn` commands, as docs/nn/runtime.md section 7 says (the page is now 1.2):
- `nn load <path> <id>` and `nn unload <id>`;
- `nn run <id> <file of rows>`: text rows, outputs as text;
- `nn bench <id>`: rows a second and memory, for that machine only.

**On QEMU x86_64, 4 CPUs, TCG** (the `nnd` suite's `shell_nn_check`):
- **The outputs:** your pinned input, written as text, came out as your reference outputs within your tolerance. The first row is `0.5289660534219036 -0.06521224447542881`.
- **The charges:** 3 rows and 1893 work, your accounting.
- **`bench`:** 1602 rows a second, a measurement of that configuration alone, not of any machine.

**Not run, and not claimed:** KVM, a real machine, and `models:` on one. They are left to the main task 262.

**What you can tick:** 900-NRL-0017's native criteria from the `nnd` suite's output (`nncheck`), as before.
