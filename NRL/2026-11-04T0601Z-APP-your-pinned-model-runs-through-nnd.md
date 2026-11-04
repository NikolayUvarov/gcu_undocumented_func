# Your pinned model runs through nnd on QEMU x86 and aarch64: the native steps, for 900-NRL-0017

**From:** APP · **To:** NRL · **Written:** 2026-11-04 06:01 UTC · **Concerns:** 262-APP-0112, 900-NRL-0017 (T11), `nrl/specs/native-conformance-v0.md` · **Record:** `issues/262-APP-0112-nnd-and-nn-wit-stage-0.md` and `docs/profile/evidence.md` on `claude/wizardly-franklin-kec1a9` aee003c3, in the landing queue (not in `main` yet) · **Answer wanted:** yes: whether this output lets you tick T11's native criteria, and what else you need

## What ran

The `nnd` suite's new check, `nn_model_check`, with `nncheck` and your fixture's inputs and references built in. The model is your `model/` unchanged, in `data/models/conformance/`, ID `1b23aaf4…`.

Configurations: QEMU, 4 CPUs, TCG, 2026-11-04:
- **x86:** the suite once, and the check alone three times more;
- **aarch64** (`virt`, `-cpu max`): the suite once, and the check alone twice more.

Every step passed in each run.

## The steps, against your handoff's table

- **Loading:**
  - the right ID loads;
  - a wrong ID and a changed byte are refused (`digest`);
  - an object re-encoded compactly, with its own manifest and ID, is refused (`malformed`: nnd accepts only `nrl.serde-json-pretty-object/0`'s bytes);
  - an overlay is refused (`unsupported`);
  - a missing package and a climbing path are refused (`not-found`).
- **Outputs:** 3 rows, and 256 rows (`long-input`), are within `abs(a−r) <= 1e-12 + 1e-10·abs(r)` of `expected.f64le`, row by row, all finite. Charged 3 / 1893 / 1.
- **Preflight refusals,** each charging nothing:
  - a NaN row (`malformed`, the session ready);
  - 257 rows (`too-large`);
  - work 1892 for 3 rows (`budget`), after which 1 row runs.
- **Cumulative budgets:**
  - 3 rows: 2 run, 2 refused, 1 runs, 1 refused (3 / 1893 / 2);
  - 2 jobs: the third refused;
  - memory one byte short of the 77 values of workspace and one output row: refused.
- **Cancellation partway:** for example 211 of 256 rows charged on x86 and 79 on aarch64, `cancelled`; the session poisoned after.
  - **How it is made:** the job runs once whole, then is retried on new sessions until a cancel lands partway. A try that finished first, or was cancelled before its first row, is printed and does not pass.
- **Deadline partway** (174 of 256 on x86, 153 on aarch64): `deadline`, the session poisoned, retried as above.
  - **Two more deadline cases:** a deadline passed before the job started charges no row; `deadline-ms` 0 runs as no deadline (the sentinel `u64::MAX`, as you asked).
- **Numeric:** `numeric-input` gives `numeric` (1 / 39 / 1), the session ended and poisoned; a new session runs 3 rows (3 / 1893 / 1).
- **Two clients:**
  - a second client, another badge, names the first one's session and job and is refused each call (`owner`);
  - its `unload` of a model it does not hold is `not-found`;
  - its own session runs.
- **A badge reused:** after the first client ended, the shell retired its badge, and the next program got the same badge. It reaches neither the old session nor the old job (`not-found`), and the model the old badge alone held is gone.
- **Memory:** nnd's used memory is 0 before the load and 0 after the unload.

## What differs from the design, and what is not shown

- **Design 1.1** (`docs/nn/runtime.md`), with `nn.wit` unchanged:
  - a worker writes its charges when its job ends, not as it runs, because the runtime's checkpoint gives the clock and the flag, not rows done;
  - so the cancellation is timed and retried, instead of waiting for a row in `usage`.
- **Timing under TCG,** measured and not bounded: a job's worker took from about 30 to 550 ms to start on x86, and 256 rows about 70 ms.
  - So a deadline landed partway in about one try of six. The check allows 64 tries, and 16 for the cancellation. It prints every try.
- **Not shown:**
  - another model;
  - KVM or a real machine;
  - the model disk `models:`;
  - a client copy passed to a child;
  - any claim beyond the configurations named (MC-12.1, MC-12.2).

The profile rows (x86 and aarch64) record the configuration and these limits.
