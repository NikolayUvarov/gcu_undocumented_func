# Requests for the assurance track (ASR), not numbered yet

**Owner:** assurance track (the assessing session, `claude/ASR-DRV`) · **Status:** open · **Recorded by:** the kernel track (KRN), 2026-10-10

The assurance track numbers its own tasks (`NNN-ASR-MMMM`), so requests from other tracks wait here. The assurance track turns each into a task and removes it from this file, and the file goes when it is empty.

## The models in Rust beside TLA+: an evaluation, then a port (500; no hurry)

**Recorded by:** the kernel track (KRN), 2026-10-10, at the maintainer's decision. **Priority: low, in the queue without hurry.**

### Problem

- **What is modelled.** The models of `docs/assurance` are checked with TLC (`tla2tools.jar`, Java), in the host part of the gates (`scripts/model_check.sh`, the group "models (TLC)"):
  - `CapRevokeMove`: the capability tree, revoke and MOVE;
  - `RevokeFlush`: the completion point of revoke on several CPUs, for x86 and aarch64;
  - the model of the kernel before issue 167, which must keep its counterexample.
- **Why ask.** The maintainer asked why Java is needed and whether it could go. There is no TLA+ model checker outside the JVM (TLC is Java, Apalache Scala), so leaving Java means leaving TLA+ for another formalism.
- **The decision so far:** TLA+, TLC and Java stay. The assurance track looks at Rust, the project's own language, without hurry.

### Plan (a proposal; the assurance track decides)

- **Evaluate a Rust model checker.**
  - For example `stateright` (MIT): models as Rust code, an explicit-state checker with breadth- and depth-first search, symmetry reduction and linearizability checks.
  - Compare with TLC on the existing models: what can be expressed, the state counts, the run times, how counterexamples read.
- **If it holds,** port `RevokeFlush` first (the smallest), then `CapRevokeMove`, each beside its TLA+ model until both agree:
  - the same invariants;
  - the same verdicts;
  - the counterexample of the model before issue 167 found again.
- **Only then** decide whether TLA+ goes. Keeping both is a valid end.
- **The models may share code with the kernel's host tests** (`tests/runtime.rs`), so a model and the code it describes are compared directly, where that helps.
- **The record.** `docs/assurance` (README), `scripts/model_check.sh`, THIRD_PARTY.md for the crate, and the CI line say what checks what.

### Acceptance criteria

- **The evaluation, written in `docs/assurance`:** what the Rust checker can and cannot express of the two models, with numbers.
- **If ported:**
  - both checkers give the same verdicts on every configuration;
  - the counterexample of the model before 167 is found by both;
  - the gate's host part runs the Rust models.
- **Java leaves the gate only when TLA+ is dropped by an explicit decision.**
