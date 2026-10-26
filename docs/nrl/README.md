# Neural Representation Lab (track `NRL`)

**Track:** `NRL`, main tasks 900–949 ([TRACKS.md](../../TRACKS.md)) · **Opened:** 2026-10-23, at the maintainer's request · **Status:** active; Codex session, `codex/NRL-lab` (the maintainer, 2026-10-25)

An extensible Rust system that inspects, converts, fine-tunes and runs trained neural networks on MIND Core and on a development host. Its first research question is whether spline representations make blocks smaller or faster than plain compression (`pruned`, `svd`) at a bounded loss of quality. A negative answer is a valid result.

## Current branch status (2026-10-26)

T06–T10 are complete for the declared bounded **host** profiles on `codex/NRL-lab`; landing is pending. **70 host tests**, portable `nrl-repr` rlibs for both targets and all CLI cycles pass. The complete check is `bash nrl/tools/check.sh`; normal Rust operation requires neither Python nor model downloads.

| Stage | Implemented and measured | Evidence |
|---|---|---|
| T06 | Actual trained TinyStories-260K decoder, custom byte-BPE, GQA/RoPE/RMSNorm/tied head and KV state; independent PyTorch/C checks | [Dense baseline](../../nrl/reports/decoder-baseline-2026-10-26.json) |
| T07 | Train-only calibration and four compressed controls, local validation selection, complete resumable state and exact continuation | [Local training](../../nrl/reports/local-training-2026-10-26.json) |
| T08 | Regression and shifted next-token metrics, repeated same-backend forward timing, resource scope and strict report compatibility | [Two-task measurements](../../nrl/reports/evaluation-2026-10-26.json) |
| T09 | Manual mixed plans, six fixed measured candidates, Pareto/rejection records and dense fallback; test after selection | [Bounded search](../../nrl/reports/plans-2026-10-26.json) |
| T10 | Two replacements, original teacher FFNs on actual student inputs, simultaneous SGD, matched original-input control, whole-model selection/error and exact saved-run replay | [Recovery](../../nrl/reports/recovery-2026-10-26.json) |

The fixed search and recovery do **not** produce an accepted compression result. Recovery changes validation NLL from 4.010611 to 4.010566, while dense gives 2.792274. Held-out perplexity is about 105.05 versus dense 9.36. The recommendation remains dense. Original dense storage is retained alongside patches; lower observed scalar forward time or fewer active parameters do not establish quality, disk/memory savings or target performance. These eight original documents form a reproducibility fixture, not a representative benchmark.

Next: **T11**, the native adapter inside APP's `nnd` and its budget/cancellation/isolation/failure contracts; **T12**, optimization of measured bottlenecks; **T13**, more model families and representations. Existing host work does not establish those native guarantees. The main task stays open. Full repository gating belongs to LANDING; NRL host development did not wait for landing.

## Execution started (2026-10-25)

The Codex session works on `codex/NRL-lab`. [Workspace and repeat commands](../../nrl/README.md), [reference inventory](reference-inventory.md), [initial block contract](../../nrl/specs/block-v5.md), and [backend ADR](ADR-0001-cpu-backend.md). Initial host results do not imply native integration or model compression. Tasks 900-NRL-0001 to 0004 are complete on the NRL branch (64 Python reference tests, 11 Rust tests, two portable release builds); code landing and KRN CI wiring are pending. Remaining representations and full-model work stay in 900.

## What is here

- **The statement:** [issues/900-neural-representation-lab.md](../../issues/900-neural-representation-lab.md), in English. It holds:
  - the goals and the requirements;
  - the architecture and the data contracts;
  - the plan (T00–T13) and the acceptance criteria.
- **The maintainer's handoff:** [HANDOFF_RU.md](HANDOFF_RU.md), version 1.0 of 2026-10-23, kept as it was written (Russian). Where it and the statement differ, the statement records why: it is read against this repository and Constitution edition 1.6.
- **The reference prototype:** `nrl/reference/qwen-spline-lab-v0.1.0.zip`, the Python/PyTorch Qwen Spline Lab v0.1.0 (MIT).
  - SHA-256 `5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25`.
  - Kept unchanged as a reference for differential tests; nothing in the system builds or runs it.

## Relations

- [262](../../issues/262-model-runtime-on-the-machines-full-hardware.md): the system's model service, in which NRL's runtime and artifacts run. NRL carries out 262's fine-tuning and spline stages.
- [178](../../issues/178-models-on-every-accelerator.md): the accelerators (`KRN`, `DRV`).

## Latest branch progress (2026-10-26)

900-NRL-0005 adds the pruned control and shared f64 channel selection. The three block fixtures, 16 host tests and both portable builds passed ([report](../../nrl/reports/pruning-f64-2026-10-26.json)). Block schema 1 replaces schema 0; regenerate older prototype artifacts. NRL development continues without waiting for landing, as the maintainer instructed. Hermite/SVD and model work remain open.

900-NRL-0006 adds cubic Hermite with linear tails, neighbour-dependent gradients and shared-channel controls: 20 host tests, two portable builds and four external cases ([report](../../nrl/reports/hermite-f64-2026-10-26.json)). Schema 2 replaces 0/1. SVD and full-model work remain.

900-NRL-0007 adds direct factor execution and a bounded Rust f64 SVD initializer. The current branch has all five bias-free representations, 25 passing host tests, two portable rlibs and five external fixture round trips ([report](../../nrl/reports/svd-f64-2026-10-26.json)). The current format is schema 3; earlier prototype versions are refused. General affine/bias contracts, model artifacts/adapters, larger-model initialization, and native execution remain open. Landing and KRN CI wiring do not block continued host development.

900-NRL-0008 adds optional affine projection biases, their gradients, preserved conversions and an explicit semantic/device applicability contract. Schema 4 replaces earlier prototypes; **31 host tests**, both portable rlibs and **12** external block cases passed ([report](../../nrl/reports/affine-f64-2026-10-26.json)). The five-representation gated SiLU numerical profile is now checked at its declared bounds; full-model contracts, T04 import/export and native execution remain open.

900-NRL-0009 imports all five original QSL overlay block representations without Python at run time, checks source/tensor hashes and semantics, and saves/reloads bounded standalone numerical block sets with explicit provenance and unrecorded channel mappings. Schema 5 is current. **37 host tests**, two portable rlibs, 12 numerical cases and the five-block import/CLI cycle passed ([report](../../nrl/reports/qsl-block-bridge-2026-10-26.json)). T04 remains open for whole-model graph/manifest contracts, base verification, QSL standalone models and full-model overlay/standalone export.

900-NRL-0010 adds the bounded `nrl.feed-forward` graph adapter and complete `nrl.model/0` packages: exact-base overlays contain only changed objects; standalone export preserves the resolved computation, tied objects, licences/notices and change history. **44 host tests**, both portable rlibs, the numerical/QSL regressions, independent PyTorch complete graphs and the full CLI cycle passed ([report](../../nrl/reports/model-graph-2026-10-26.json)). The standalone runs after its base files are removed. Next is T05's independent model dataset/task/training pipeline; full-graph backward, pretrained-model quality/compression and native execution are not claimed. QSL whole-language-model import and other-family T04 work remain open.

900-NRL-0011 completes the first independent regression task profile: full-graph backward and tied-object SGD, declared train/validation/test groups, train-only gradients, earliest validation checkpoint selection and post-selection test evaluation. The initial model, full dataset/config, actual Rust learning curve, training provenance and selected standalone package are saved in a [replayable example](../../nrl/reports/regression-example-2026-10-26/report.json). **52 host tests**, both portable rlibs, all previous regressions and the training/check/replay CLI cycle passed ([dated evidence](../../nrl/reports/regression-training-2026-10-26.json)). The external PyTorch history chooses epoch 91 of 128, as Rust does. This is T05's first complete regression profile; next are T06's real language adapter and T07's calibration/local training. Classification, optimizer resume, mixed plans/search, benchmarking, useful compression and native execution remain open.

900-NRL-0012 completes T06 for a real trained TinyStories-260K/custom-tokenizer profile: full scalar f64 decoder, GQA/RoPE/RMSNorm/tied head, byte-BPE, model-bound KV state and bounded standalone packages. **57 host tests**, both portable rlibs, independent actual-weight PyTorch f64/original C float32/tokenizer checks and CLI import/check/probe pass ([dense baseline](../../nrl/reports/decoder-baseline-2026-10-26.json)). Original dense storage is retained; no held-out language or compression result is implied. T07 calibration/checkpoints and T08–T10 evaluation/plans/adaptation continue autonomously at the maintainer's explicit request.

900-NRL-0013 completes T07 for the bounded trained decoder: split-aware original-teacher train/validation pairs, four Rust compressed initializers, validation-only local SGD and exact serialized optimizer/data/order/current/best continuation. **62 host tests**, both portable rlibs and calibration/train/resume/export CLI cycles pass ([evidence](../../nrl/reports/local-training-2026-10-26.json)); the [saved example](../../nrl/reports/local-example-2026-10-26/report.json) includes actual calibration and partial/complete checkpoints. Test activations do not initialize/train/select. The six T09 plans, quality/package budgets and T10 protocol/rounds were frozen before T08 test evaluation. T08–T10 continue next; local errors do not establish model quality or compression.

T08 is complete on the NRL branch (900-NRL-0014): 65 host tests, both portable rlibs, two independent task metrics and compatible repeated timing reports. [Evidence](../../nrl/reports/evaluation-2026-10-26.json). Original dense storage and whole-process RSS are reported honestly; the current linear student loses language validation quality. T09/T10 continue using the unchanged pre-frozen study.

T09 is complete on the NRL branch (900-NRL-0015): 68 tests and both portable rlibs; manual mixed composition and six fixed candidates were checked whole. [Measured search](../../nrl/reports/plans-2026-10-26.json) selected `dense` under the validation quality/time/package rules; negative cases are retained. Test follows selection. T10 student-input recovery continues independently.
