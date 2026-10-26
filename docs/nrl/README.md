# Neural Representation Lab (track `NRL`)

**Track:** `NRL`, main tasks 900–949 ([TRACKS.md](../../TRACKS.md)) · **Opened:** 2026-10-23, at the maintainer's request · **Status:** active; Codex session, `codex/NRL-lab` (the maintainer, 2026-10-25)

An extensible Rust system that inspects, converts, fine-tunes and runs trained neural networks on MIND Core and on a development host. Its first research question is whether spline representations make blocks smaller or faster than plain compression (`pruned`, `svd`) at a bounded loss of quality. A negative answer is a valid result.

## Current branch status (2026-10-26)

T06–T10 are complete for the declared bounded host profiles. The latest branch has **82 host tests**, four bare-metal rlib builds (`nrl-repr` and `nrl-runtime`, both targets) and all CLI cycles passing. Run `bash nrl/tools/check.sh` from the repository root to repeat them. Code landing/full repository gating remains pending.

| Work | Current scope and outcome | Evidence |
|---|---|---|
| T06–T10 | Real trained decoder/tokenizer, resumable calibration/local training, two-task metrics, mixed-plan search and student-input recovery | [Recovery and previous work](../../nrl/reports/recovery-2026-10-26.json) |
| T11 — partial | Portable no_std/no_alloc graph executor, owner/model-bound session state, cumulative budgets, cooperative deadline/cancel controls and verified host bridge | [Host preparation](../../nrl/reports/native-boundary-2026-10-26.json) |
| T12 — first host experiment | Immutable FFN views/scratch reused per document; full forward promoted after fixed paired measurements; no numerical/quality change | [Pre/post measurements](../../nrl/reports/optimization-2026-10-26.json), [final build](../../nrl/reports/optimization-final-build-2026-10-26.json) |
| T13 — first host extension | Independent mean-token embedding family, shared five-path projection, standalone export/pruning, Recall/MRR/NDCG and shared evaluation/benchmark/compare | [Independent reference and example](../../nrl/reports/embedding-2026-10-26.json) |

The prepared decoder's recorded paired medians are about 5.8–5.9 ms for the original path and 4.1 ms for preparation (about 30% less elapsed time), with zero observed logit/argmax differences and fixed quality limits met. Each report records its executable/host/workload; these are scalar host observations, not native/device/generation guarantees. Source weights and numerical policy are unchanged. The original checked path remains available.

The compression study still recommends dense: recovered-student held-out perplexity is about 105.05 versus dense 9.36 on the miniature corpus. T12 improves execution of the same model without changing its weights. The embedding/retrieval fixture is original synthetic data and does not establish representative semantic quality, pretrained quality or accepted compression benefit.

**T11 remains open.** APP owns the absent `nnd`/`idl/nn.wit`; the [request](../../issues/requests-APP.md#nrl-native-nnd-integration-for-900-t11) records native parser/client authentication/memory/time/worker/cancellation and named-target test requirements. The portable graph profile does not cover native decoder KV or native embedding execution. Cross-builds and logical host sessions are not OS isolation/native execution evidence. Further T12 SIMD/parallel/device work needs its target measurements; broader T13 families and full embedding lookup/pooling/normalization training remain separate extensions. Parent 900 stays open; deferred landing does not block independent host work.

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
