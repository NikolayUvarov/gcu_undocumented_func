# Ready: five autonomous NRL host extensions

**From:** NRL · **To:** LANDING · **Written:** 2026-10-26 19:38 UTC · **Answers:** — · **Concerns:** 900-NRL-0020–0024; native 900-NRL-0017 remains open · **Answer wanted:** normal landing result

**Ready:** `codex/NRL-lab` 11547f61a75d6d1efb5c001439cfccb7cd3d3189

Supersedes the unlanded NRL request `LANDING/2026-10-26T1829Z-NRL-ready-runtime-host-optimization-and-embedding-retrieval.md` (b47b72cd). The maintainer has deferred landing and requested autonomous host work; this updates readiness without treating the landing date as a development blocker.

The branch includes current main bcb4549281dc9cadde3eab79ec5be47c3fe78d89. Source b44fbc0d2ac16e293175ec0611511898a1678713 passed the complete `bash nrl/tools/check.sh --expanded` profile on the development Linux x86-64 host: **95 Rust tests, zero failures/skips**, all CLI cycles, and release builds of both `nrl-repr` and `nrl-runtime` for x86_64-unknown-none and aarch64-unknown-none-softfloat. Ready HEAD adds only issue/documentation/evidence/notice files; source, tools, fixtures, Cargo and THIRD_PARTY are unchanged from that tested source. The branch is also staged in fast-test 534bd6d8; nothing was built/tested there.

Closed within explicit bounded host scopes:

- 900-NRL-0020: shared AdamW, complete parameter-bound moments, exact checkpoint continuation, refreshed student-trajectory local FFN fitting and matched SGD control. The mixed compressed model still fails the frozen quality budget.
- 900-NRL-0021: prepared incremental sessions and greedy generation, reference preserved, identical full step logits/token IDs, rollback after late numerical failure. Fixed paired study accepted default promotion; final host request medians 14.248 ms reference versus 9.985 ms prepared. An intermediate overlapping timing repeat is preserved; no general/native speed promise.
- 900-NRL-0022: full lookup/mean/projection/L2 backward and graded contrastive AdamW with partial/resumed checkpoints, validation selection and standalone export. A distinct trained-table retrieval task preserves the original projection-only restriction. Synthetic contrastive losses improve; NDCG does not. All index documents participate; only query splits are held out.
- 900-NRL-0023: complete executable QSL overlay/base binding and standalone export for the exact supported llama2c/Tinyllamas adapter, checked with unchanged QSL writer plus independent full PyTorch logits/greedy tokens. HF/Qwen fingerprints/architectures and external HF standalone imports remain unsupported.
- 900-NRL-0024: pinned attributed CDLA-Sharing-1.0 TinyStories source, 48 modified story prefixes (24/12/12), eight frozen plans plus two recovery arms, validation-only selection and diagnostic held-out reports. Repeated final-source outputs match first-run model IDs/numerics exactly; dense remains selected. Licence, prominent modifications, publisher card and source/input hashes are recorded.

New optional-reference generators reproduce byte-identically under the pinned PyTorch environment (QSL's environment metadata is excluded from its cross-host core-manifest comparison as specified). Rustfmt and changed-document links pass.

Evidence on the ready branch:

- `nrl/reports/five-extensions-2026-10-26.json` and `five-extensions-check-2026-10-26.txt`.
- `nrl/reports/expanded-study-final-2026-10-26/summary.json`; saved initial/final studies, checkpoints and complete model packages.
- `nrl/reports/generation-final-build-2026-10-26.json` and all pre/intermediate measurements.
- `nrl/reports/embedding-training-example-2026-10-26/` and `qsl-model-example-2026-10-26/`.
- Current status and exact boundaries in `nrl/README.md`, `docs/nrl/README.md`, parent 900 and the five closed issues.

No kernel, ABI, service IDL, native integration or OS profile claim changes. Native T11 remains APP-blocked by the recorded nnd/nn.wit request. No accepted compression, native decoder/embedding, SIMD/parallel/GPU, LoRA/QLoRA, mixed-precision or full decoder-backprop result is claimed. Parent 900 stays open. NRL checks do not replace your full repository gate; until KRN's CI wiring lands, run the explicit NRL script as well.
