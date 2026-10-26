# Ready: trained decoder, measurement, mixed plans and student-activation recovery

**From:** NRL · **To:** LANDING · **Written:** 2026-10-26 14:37 UTC · **Concerns:** 900-NRL-0001 through 0016; 900 T06–T10 · **Answer wanted:** no immediate answer; landing remains deferred by the maintainer

**Ready:** `codex/NRL-lab` ab17bb848c91df5607bcdfd16399f857c5b1e861

Supersedes `LANDING/2026-10-26T1152Z-NRL-ready-regression-training-and-replay.md` and all older unanswered requests for this branch; includes their changes. Closes 900-NRL-0012 through 0016 on the NRL branch. Parent 900 remains open. Main `f4082b533dbc3f3f6d64aff7336a4ab1c01576f4` is an ancestor. Latest executable sources/scripts/fixtures/dependencies/THIRD_PARTY.md were checked at `5edd9bc6`; the final commit adds saved evidence/models and status/usage documentation. This remains a code/data request requiring the full landing gate, not the text-only exception.

## Implemented profiles

- T06: real permissively licensed trained TinyStories-260K model and its separate custom 512-token byte-BPE tokenizer, pinned by upstream revision/size/hash/notice. Bounded full scalar f64 decoder (GQA, stored RoPE, RMSNorm, tied head), model-bound causal state, exact source identity, patched standalone packages. Independent actual-weight PyTorch f64, original C float32 and tokenizer oracles are separate evidence profiles. No C/Python is required for normal Rust operation.
- T07: split-aware train/validation calibration, four compressed initializers, train-only channel/grid construction, local full-batch SGD, earliest validation selection, complete saved initial/current/best state and exact optimizer/data/order continuation. No test activations train or select.
- T08: complete regression and next-token task metrics, independent evaluation/benchmark/comparison CLI commands, monotonic warmups/repeats/min/median/p95, actual work, exact backend/host/executable/task/data/split/measurement compatibility and explicit effective/retained/package/whole-process RSS scope.
- T09: strict manual mixed plans and bounded six-plan search against dense, full-model validation measurements, Pareto/refusals, quality/package/timing rules and dense fallback. Selected test evaluation follows selection. All plans/settings were frozen before test evaluation.
- T10: two simultaneous replacements adapted using actual train-only student inputs, frozen original teacher FFNs on those same inputs, matched fixed-original-input control, all rounds including zero, accumulated full-logit error and earliest full-model validation selection. Hash-bound pairs/deterministic records/standalone models and independent recovery/check/replay tools recompute the entire run. This is local FFN recovery, not end-to-end decoder backpropagation.

## Validation and actual outcome

`bash nrl/tools/check.sh` passed **70 Rust host tests**, portable release rlibs for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat`, all earlier numerical/QSL/regression checks and complete import/calibration/train/resume/export/two-task benchmark/compare/manual-plan/search/recovery/check/replay CLI cycles. Saved final recovery was also checked independently after export. Build/test/gate ran only on `codex/NRL-lab`; no test/build/gate ran on fast-test. Built source commits and final evidence were merged/pushed to fast-test in the required order.

**Negative results are retained.** Six-plan search selects dense under the fixed quality budget. Three student-input rounds improve validation NLL from 4.010611310636807 to 4.010565653753475; original-input control gives 4.010594939048665. Dense gives 2.792273884144422. Held-out perplexity is 105.0541886201988 for recovered student versus dense 9.35922873023494. The deployment recommendation stays dense even though this scalar run observes lower forward time. No accepted compression, representative language quality or native performance gain is claimed.

## Evidence and repeatability

Dated reports: `nrl/reports/decoder-baseline-2026-10-26.json`, `local-training-2026-10-26.json`, `evaluation-2026-10-26.json`, `plans-2026-10-26.json`, `recovery-2026-10-26.json`; corresponding raw check logs are committed. Actual standalone models, corpus/settings, local checkpoints, six-plan measurements and both arms' actual recovery pairs/histories/reports are under `nrl/reports/*-example-2026-10-26/`. `nrl/README.md` gives fresh-output and saved-run repeat commands. Schema/bounds/protocols are in `nrl/specs/decoder-v0.md`, `calibration-local-v0.md`, `evaluation-v0.md`, `mixed-plans-v0.md`, `student-recovery-v0.md`.

Frozen study file SHA-256 remains `da512b009bf26720d4014a02f252995fa6521d1446d45f059ad023748ee3fab2`. Deterministic final recovery record ID: `e18be99405280d17a0e8fea21ff26d47e943c4be923126eaca14e8e7b3a050b6`. Original dense blobs remain retained alongside patches; package bytes can grow and process RSS is never attributed as model savings. Binary reference/package files are explicitly tracked despite the general *.bin ignore; no proprietary files are added.

**Integrator:** explicitly run `bash nrl/tools/check.sh` on the exact candidate until KRN wires the workspace into CI (`issues/requests-KRN.md`); this remains necessary alongside the full repository gate. The portable rlibs are not native execution evidence. No ABI, IDL, kernel, APP-owned service or CI implementation was changed by NRL. T11 native nnd integration, T12 measured optimization and T13 expansion remain. The maintainer asked NRL to finish through T10 independently of deferred landing; no expedited gate or immediate response is requested.
