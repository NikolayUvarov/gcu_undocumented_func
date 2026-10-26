# 900 — Neural Representation Lab (NRL): an extensible Rust system that inspects, converts, fine-tunes and runs trained neural networks, with spline representations as a hypothesis tested against plain compression

**Type:** main task of the track `NRL` · **Owner:** Codex session, track `NRL`, `codex/NRL-lab` (the maintainer, 2026-10-25) · **Priority:** P1 · **Status:** open · **Blocked by:** — for T00–T10 on the host; T11 by the APIs of [262](262-model-runtime-on-the-machines-full-hardware.md) and its native platform adapter (memory/worker APIs: 262-KRN-0160 to 0164) · **Roadmap:** track G (user services); Article 8 (the cognitive plane); tracks A and H through [178](178-models-on-every-accelerator.md) · **Constitution:** MC-3.1, MC-3.3, MC-3.13, MC-5.1, MC-5.2, MC-8.1, MC-8.5, MC-9.1, MC-11.1, MC-11.2, MC-11.3, MC-11.5, MC-11.8, MC-11.11, MC-12.1, MC-12.2, MC-12.3

**Recorded by:** the tools track, 2026-10-23, at the maintainer's request: "add this task as a separate track for neural networks". The source is the maintainer's handoff of 2026-10-23, version 1.0. It is kept as it was written, in Russian, in [docs/nrl/HANDOFF_RU.md](../docs/nrl/HANDOFF_RU.md). This issue is its statement in English, read against this repository: the Constitution of edition 1.6, the APIs that exist and the tasks that already plan the hardware.

## 1. Context and goal

- **The user has or will have trained weights of many networks.** The research idea is to see the transformations of internal states as motion in a many-dimensional space, and to look for a more compact description of them. The candidates already considered:
  - one-dimensional splines;
  - fewer intermediate channels;
  - low-rank factorizations;
  - local distillation;
  - table lookups.
- **The practical goal:** a smaller model, less memory and/or less time per run, at an explicitly bounded loss of quality.
  - Fine-tuning and approximation are allowed.
  - No single number for the allowed loss is fixed yet: each experiment's profile states its own.
- **Not a goal: to remove every matrix product whatever the result.** A compact model must run as it is. Rebuilding the original large matrices before each run is not a speed-up.
- **The Kolmogorov–Arnold theorem is no promise** of a cheap exact conversion of an arbitrary trained network. The spline representation is a hypothesis to be tested. A negative result for splines is a valid result, and it stays in the reports (MC-12.3).

## 2. What is a requirement and what a proposal

| Statement | Status |
|---|---|
| Support many networks; Qwen is only an example | The maintainer's direct word |
| The target operating system is written wholly in Rust (MIND Core) | The maintainer's direct constraint |
| Fine-tuning and some loss of accuracy are allowed | The maintainer's direct permission; numerical thresholds per experiment profile |
| Separate tools for download, conversion and checking; a choice of representations | The project's composition asked for earlier; kept |
| A portable Rust core and an unprivileged service in the OS | Proposed architecture, consistent with the Constitution's split of kernel mechanisms and application environments |
| Rust tools for preparation and training, not only a runtime | The proposed final state of the whole process; introduced stage by stage |
| `no_std` + `alloc` for the minimal compute profile | A proposed portability criterion, not a ban on `std` in host tools |
| Burn (and its CPU backend) as a starting library | A candidate proposed earlier; portability and dependencies still to be checked |
| Qwen3-0.6B-Base as the first language reference | A proposed experiment profile, not a model the whole project must use |
| A Wasm interface, GPUs, quantization, lookup tables | Possible extensions; never to be presented as done |

The working name NRL does not mean the old package is renamed or its format broken. The original prototype stays as a separate reference.

## 3. What already exists

### 3.1 The reference prototype

- **The file:** `qwen-spline-lab-v0.1.0.zip`, SHA-256 `5e27aab03013e16ac447f12e7093b39ca36167fb20b9b74368661ea425a38b25` (checked again on 2026-10-23 on its upload; the archive is whole).
  - It is to be kept in this repository as `nrl/reference/qwen-spline-lab-v0.1.0.zip`, unchanged, under its MIT licence ("Qwen Spline Lab contributors"), and recorded in [THIRD_PARTY.md](../THIRD_PARTY.md).
  - It holds no third-party code and no model weights.
- **What it is:** a Python/PyTorch prototype for the MLP blocks of the dense text Qwen3. It has a loader, calibration, conversion, local training, block and language-model checks, speed measurement, generation and export.
- **Its five representations:** `dense`, `pruned`, `spline-linear`, `spline-cubic`, `svd` (its README, [S1]).
- **Its recorded test protocol** (22 October 2026): **59 passed, 1 skipped** (the Transformers integration). Numerical checks and a synthetic cycle ran.
  - That is not a new run, and no evidence for Rust or for the quality of a compressed Qwen.
  - Trained Qwen weights, a full language experiment, GPU speed and an export of the real Hugging Face graph were not checked there ([S2]).
- **The map of what to port:**

| File of the reference | What it holds / where it goes |
|---|---|
| `src/qwen_spline_lab/representations.py` | `ChannelSpline`, `CompactGatedMLP`, `LowRankLinear`, `SVDGatedMLP`: construction, channel choice, initialization, refit |
| `artifacts.py` | Saving and loading an overlay, checking a calibration |
| `training.py` | Block metrics and local training |
| `evaluation.py` | Language metrics, benchmark, generation, report comparison |
| `hf.py` | The Qwen/Transformers adapter; it must not go into the general core |
| `workflows.py` | The wiring of conversion, training and testing |
| `tests/` | Numerical and format checks: the basis of Rust's differential tests |

- **Its limits:** the prototype is tied to Qwen and to language metrics, and deliberately refuses other model types. Renaming it does not make a multi-model architecture ([S2]).

### 3.2 What MIND Core has today (read from this repository)

- **`mind::nn`** (`libmind/src/nn`) interprets ONNX graphs from `MINDNN01` files on the CPU, with SSE and AVX2 matrix products, for dictation (250) and speech (252).
- **[262](262-model-runtime-on-the-machines-full-hardware.md)** (`APP`) plans the system's model service: `nnd`, `idl/nn.wit`, `nn`, budgets, safetensors/GGUF/ONNX readers, and the CPU and GPU runtime. Its stages 4 (fine-tuning) and 5 (splines) are taken over by this track (below, section 12).
- **[178](178-models-on-every-accelerator.md)** (`KRN`, `DRV`) plans the accelerators: all of the CPU, one accelerator interface, then the AMD GPU, Intel's NPU and the NVIDIA RTX.
- **The kernel** saves the AVX, AVX-512 and AMX state of each task (174-KRN-0037).
- **Platform inventory updated 2026-10-25 (main dc2e4312):**
  - ABI 20; rebuild programs against the current `common/abi.rs` and `libmind`.
  - A program named by the owner in `data/memory.conf` can have a quota up to 64 GiB ([262-KRN-0160](../issues-done/262-KRN-0160-large-private-memory-for-a-named-program.done)); the ordinary default remains at most 1024 MiB.
  - Worker processes share explicitly granted memory objects ([262-KRN-0163](../issues-done/262-KRN-0163-workers-on-several-cpus.done)); these are not shared-address-space threads.
  - Stacks can be 64 KiB to 8 MiB ([262-KRN-0164](../issues-done/262-KRN-0164-stacks-larger-than-64-kib.done)).
  - `vfs.wit` 2.4 `fill` reads a file into a lent memory object ([262-KRN-0162](../issues-done/262-KRN-0162-a-file-read-once-into-a-memory-object.done)); it is not arbitrary file-backed mmap.
  - Large pages are implemented, with hardware measurements still open ([262-KRN-0161](262-KRN-0161-large-pages.md)). Recorded evidence above is QEMU's, not a native NRL run.
  - `nnd` and `idl/nn.wit` are still planned by APP; host T00-T10 need neither.

## 4. The bounds of the first version, and later growth

- **The first port must show three things:**
  - the numerics are right;
  - at least one real model works;
  - the shared components do not depend on that model's name.
- **The first set of objects:**
  - synthetic blocks;
  - a compact ordinary feed-forward network for classification or regression;
  - one small decoder transformer, for example Qwen3-0.6B-Base.
  - The second, independent architecture checks extensibility. It does not promise every network at once.
- **Later:** other transformers, encoder and embedding models, CNNs, MoE, recurrent and multimodal networks.
  - Each family declares on its own its operator coverage, its allowed transformations and its evaluation profile.
  - An adapter for one family is no support of another.
- **Not in the first version without a separate decision:**
  - a universal compiler of all ML graphs;
  - full training of large networks from scratch;
  - dynamic execution of arbitrary code from model repositories;
  - a GPU driver of its own (that is 178's);
  - an automatic exact conversion of any network into a KAN.

## 5. The architecture: five independent parts

```text
                    Model and data sources
                               |
                  Importer / download provider
                               |
            ModelAdapter + description of the supported graph
                               |
              Analysis -> Calibration -> Transformation plan
                               |
               Representation / Transform registry
                               |
                 Fine-tuning -> Full evaluation -> Selection
                               |
                 Versioned model artifact
                               |
                     Portable Rust runtime
                               |
                    ComputeBackend / Platform
                               |
               Host profile         MIND Core profile
```

**5.1 ModelAdapter: what one architecture knows.**
- **What it does:**
  - reads the configuration and the weights, and fixes their identity;
  - defines the inputs, the outputs and the state;
  - lists the candidates for replacement;
  - runs the original model and applies a plan of replacements.
  - A tokenizer or another preprocessor belongs to the adapter or the task profile, not to the general optimizer.
- **Never in shared code:** `qwen`, the number of layers, head sizes, the activation type, tensor names, the KV-cache scheme.
- **Kept by the architecture's contract:** tied weights, biases, the order of normalizations, masks, positions and state transitions.
- **An unsupported operation is a diagnostic error.** It may be left untransformed only when the executor runs the original operation correctly. "Not compressed" and "cannot run" are different states.

**5.2 Representation / Transform: how a block changes.**
- A transform declares:
  - which semantic block type it applies to;
  - what calibration data it needs;
  - how its parameters are initialized;
  - whether it trains;
  - which operators it needs at run time.
- **Explicit block types** (affine, linear, plain feed-forward, gated feed-forward, and others really supported), instead of assuming that every MLP is a SwiGLU without bias.
- Applicability also depends on dtype, shapes, device, state and whether a backward pass exists.

**5.3 ComputeBackend: running operations.**
- It owns the computation, dtypes, device memory, synchronization and the declared hardware capabilities.
- A model's representation holds no pointers to a backend's functions.
- **An unsupported pairing of representation and device** is an error, or a fallback the user chose explicitly. Never a hidden switch to the CPU.
- **The first version needs a CPU profile only.** A GPU is a separate integration, once 178's stack exists.

**5.4 Platform: the OS integration.**
- It owns access to objects, memory, time, job execution, cancellation and resource accounting.
- The host adapter may use an ordinary OS; the target adapter uses MIND Core's own services.
- A file path is not the only way shared libraries address a model.

**5.5 TaskProfile / Evaluator: what quality means.**
- It fixes the data, the preprocessing, the metrics, the thresholds, the direction of improvement, the length of measurements and the rules of comparability.
- Perplexity is no general metric for every network.

The first registry may be static, at build time. Extensibility through Rust traits and versioned schemas needs no dynamic libraries yet. Rust's internal ABI is never an implicit stable protocol between domains (MC-11.1).

## 6. Minimal data contracts

| Contract | What it must hold |
|---|---|
| ModelDescriptor | Family, adapter version, configuration, source hashes, tied weights, preprocessor, supported tasks |
| BlockDescriptor | Stable ID, semantics, inputs and outputs, dtype/layout, parameters, state, allowed dynamic sizes |
| TransformSpec | Representation ID and version, block ID, width/rank/grid, bias and activation rules, operators needed |
| CalibrationRecord | Model and block ID, inputs, target outputs, masks/context/state where needed, split and provenance |
| ExecutionProfile | Backend, target, numerical policy, limits of shapes, memory, sessions and time |
| ArtifactManifest | Source model, the full plan of replacements, tensors, dependencies, schemas, provenance and runtime requirements |
| EvaluationReport | Data, versions, metrics, actual work done, environment, precision, repeats, errors and omissions |

No universal IR first. A narrow registry of block types and a description of supported graphs are enough. Their bounds must let a new family be added later without changing the already checked mathematics of splines or SVD.

## 7. Representations: the required core and the research

| Representation | Semantics of the first version | Acceptance |
|---|---|---|
| `dense` | The block's function and shape unchanged | Reference equivalence and round trip |
| `pruned` | Fewer internal channels, the supported block's activation kept | Consistent channel choice, sizes, bias and training |
| `spline-linear` | A reduced block with per-channel piecewise-linear functions | Knots, interpolation, linear tails, forward and backward |
| `spline-cubic` | Per-channel Hermite interpolation | Consistent derivatives on segments, the same tail contract |
| `svd` | Factorization of supported linear/affine projections | Reconstruction, factor orientation, bias, rank limit |

This ports the semantics of the reference ([S1]). For a gated MLP:

```text
F(x)      = W_down [ activation(W_gate x) * (W_up x) ]
F_hat(x)  = U      [ spline(V_gate x)     * (V_up x) ]
```

- **Other block types need their own correct variants.** No original activation is silently replaced by SiLU.
- **Specified and tested:** the grid's parameters, the matrices' orientation, the rounding of the interval index, and the behaviour at the ends and beyond the range.
- **The cubic variant of the reference** is Hermite interpolation with derivatives from neighbouring knots, not a B-spline ([S1]). It is not changed in the same step as the language.
- **The research backlog** (each method needs its own quality check and a direct executor):
  - activation-aware low rank;
  - merged bottlenecks;
  - grouped splines;
  - quantization;
  - product quantization and lookup tables;
  - conditional skipping of blocks.
- The reference's plain SVD is not to be called SVD-LLM ([S1]).

## 8. Calibration, training and choice of representations

- **Data is split into train, validation and test before any activation is collected.**
  - The unit of the split follows the task: a document, an object, a user, a video sequence, or another source of dependence.
  - Exact duplicates and known related groups never cross parts.
- **The first local training:**
  - fix the original teacher, and collect input/output pairs of the block;
  - the replacement may be trained apart from the full model, with its matrices and coefficients optimized;
  - the best checkpoint is chosen by validation.
  - Test never chooses a width, a rank, knots or a step count.
- **The local block's NMSE** is normalized by the energy of its own target output, not by the large residual vector.
  - Set an epsilon and a report for near-zero energy.
  - Also record the absolute error, the distribution of errors and how often inputs leave the grid.
- **The splines' benefit** is checked against a reduced block without splines, at a comparable number of parameters, data and training budget, with comparable initial channels and seed. Compression by itself does not show that splines help.
- **After the local test the full model is checked.** With several replacements, the change of activation distributions counts.
  - Calibration on the states of the already transformed student, with explicitly defined teacher targets, then joint distillation or recovery fine-tuning, is a separate task.
  - The reference v0.1 has none of these ([S1]).
- **The automatic choice** neither just picks the least local MSE nor minimizes each block alone. The first procedure:
  1. a bounded enumeration of candidates;
  2. a local filter;
  3. a full validation evaluation;
  4. a measurement on the target platform;
  5. a Pareto set over quality, memory and speed.
  - The final combination of blocks is always checked as a whole.
  - "No improving variant found" is a correct result.
- **For training in Rust:**
  - check autodiff on the chosen backend;
  - a specialized representation may have its own backward pass, but only with independent gradient checks;
  - resuming needs the optimizer's state, the RNG, the step and the data order. Loading the weights alone is no exact resume.

## 9. Rust and portability

- **The target runtime and the native integration are Rust.** The Python prototype stays an auxiliary reference; its absence must not stop a finished native model from running. The whole preparation chain also moves to Rust step by step, never through a hidden Python call from a "Rust CLI".
- **The proposed workspace,** under `nrl/` in this repository:

```text
nrl/
  crates/
    nrl-core/               # types, block descriptions, contracts
    nrl-format/             # schemas, validation, tensor objects
    nrl-repr/               # mathematics of the representations
    nrl-adapters/           # the model families' modules, apart
    nrl-runtime/            # graph execution and session state
    nrl-calibrate/          # recording and checking calibration
    nrl-optimize/           # initialization and the transformation plan
    nrl-train/              # training and checkpoints
    nrl-eval/               # task profiles and comparison
    nrl-backend-cpu/        # the CPU implementation or a library's adapter
    nrl-platform-host/      # the development environment
    nrl-platform-mindcore/  # the target OS
  tools/
  tests/fixtures/
  specs/
  reports/
  reference/                # qwen-spline-lab-v0.1.0.zip, unchanged
```

  - For a small first pull request some crates may be merged. The bounds of dependencies matter, not the number of crates.
- **The compute core** is designed for the minimal profile `no_std` with `alloc`:
  - no network, file paths, threads or system clocks inside the mathematical operations;
  - inputs, outputs and large working buffers passed explicitly;
  - a controlled refusal when resources run out (MC-5.2).
  - Host tools may use `std`.
- **Libraries:** Burn (with its CPU backend) was considered as a candidate, Candle as an extra base for studying model implementations. Neither is shown to work on MIND Core.
  - A spike is needed: the exact revision, features, the dependency graph, a build for the target, forward and backward of a simple block, and an audit of FFI and system calls (MC-11.8).
  - The chosen backend is fixed by an ADR with its evidence.
- **If a library does not fit,** the OS is not ported to it wholesale. Make a narrow backend, or a small CPU path of the track's own.
  - GPU/BLAS and other outside implementations only in an explicitly named profile, never silently in the required all-Rust profile.
- **SIMD:** check the hardware's capabilities and that their state is saved across context switches (on MIND Core: 174-KRN-0037).
  - The kernel's compiler settings are not inherited by a user compute process without analysis.
  - `no_std`, Rust and fewer parameters are by themselves no evidence of speed or of hard real time.

## 10. Formats and import

- **The QSL artifact is imported as a bridge first,** but a versioned general manifest is designed from the start.
  - A tensor format (the reference's safetensors) does not stand in for a description of the computation graph.
- **Two distinct kinds of result:**

| Kind | Semantics |
|---|---|
| Overlay | Only the changed blocks; the exact source weights are required |
| Standalone | Every weight, transformation and piece of metadata needed; no source folder required |

  - A small overlay is never presented as the size of the full model.
  - Tied weights are counted without artificial duplication.
  - The schema's version, the adapter's version, the transformations and the numerical policy are recorded apart.
- **The loader checks:**
  - sizes, allowed dtypes and layouts, offsets, overflow of size arithmetic;
  - references to tensors, finiteness and ordering of grids;
  - limits on the manifest's size, the number of objects and the volume to unpack or parse.
  - A hash confirms the bytes' identity, not that a trusted author made them; authenticity is a separate policy (MC-9.1).
- **Never executed:** pickle, remote Python, arbitrary install scripts, or code from weights (MC-11.3, MC-11.11).
- **Kept:** source identifiers, licences and required notices.
- **Offline:** for a local model, import and every later stage work without a network. The native runtime never downloads a missing dependency by itself.

## 11. Tools and a proposed command line

These names are an interface design, **not commands that exist**. Each operation has a binary or an independent entry of its own, and a shared library function. A series runner does not replace the separate tools.

| Proposed command | Purpose |
|---|---|
| `nrl-doctor` | Check the target's and the backend's capabilities and the dependencies |
| `nrl-download` | Download a model through a chosen provider and pin its revision |
| `nrl-import` | Check local weights or an artifact, without a network |
| `nrl-inspect` | Show the architecture, candidates, compatibility and memory needed |
| `nrl-split` | Prepare independent data parts with provenance |
| `nrl-calibrate` | Collect inputs, outputs and the states needed |
| `nrl-convert` | Apply a manual transformation plan |
| `nrl-train` | Fine-tune the chosen blocks |
| `nrl-block-test` | Local errors and range coverage |
| `nrl-evaluate` | Full evaluation by a task profile |
| `nrl-benchmark` | Measure time, memory and the actual amount of work |
| `nrl-search` | A bounded enumeration of candidates within a budget |
| `nrl-compare` | Compare compatible reports and build a Pareto set |
| `nrl-export` | Make an overlay or a standalone artifact and a provenance report |
| `nrl-run` | Run a model; generation is a mode of a suitable adapter only |

- **Each command:**
  - returns a clear exit code and keeps a machine-readable JSON report;
  - runs no hidden stage, and overwrites no data without explicit leave.
- **On MIND Core** the same operations are typed requests to a service, not a POSIX shell.
- **A transformation plan** holds the model ID, block selectors, representations and their parameters, train and validation sources, the budget, the numerical policy and the quality limits.
  - Numbers such as `width=1024`, `rank=256`, `knots=16`, `layer=14` belong to a concrete Qwen example only, never to general defaults.

## 12. Integration with MIND Core

- **The result is an unprivileged service, not an extension of the kernel** (the Constitution's split of kernel mechanisms and application environments, Articles 1, 2 and 11; the handoff read edition 1.3, this repository has 1.6).
- **One model service on the system.** NRL's runtime crate and its artifact format run inside [262](262-model-runtime-on-the-machines-full-hardware.md)'s `nnd` and `idl/nn.wit`, so that the system has one service that holds models, budgets and devices, instead of two.
  - 262 keeps the service, the interface, the standard models' runtime and the accelerators through 178.
  - NRL owns representations, conversion, calibration, fine-tuning, evaluation, the artifact format and its runtime crate.
  - 262's stages 4 (fine-tuning) and 5 (spline networks) are carried out by NRL (T07, T10, T03); 262 keeps a reference to them.
  - If the two tracks' owners agree otherwise, they record it in both issues.
- **Authority** (MC-3.1, MC-3.3, MC-3.13, MC-5.1):
  - the service is given access to the specific model objects, memory, a compute budget and the right to use a device;
  - the importer, training, publishing a new version and ordinary execution may hold different authorities;
  - an ordinary inference session needs no network.
- **The cross-domain contract** defines:
  - the request's schema, input sizes, queue bounds and backpressure;
  - cancellation, errors and completion;
  - who owns each buffer.
  - Only checked, stable data reaches the compute core.
- **Sharing:** large read-only weights may be shared only under an immutability contract. The KV cache and other mutable state are isolated per session.
- **No IPC per scalar operation.** Computation runs in the service's domain; results or bounded streams go out. A Wasm application may call this interface, but Wasm is no required form of the numerical core.
- **Versions** (MC-8.5):
  - a session is bound to a version of the weights and the configuration; a KV cache is never reused after the model changes without proven compatibility;
  - training makes a new immutable artifact, whose publication is checked and authorized;
  - rolling a model back does not undo external actions already taken.
- **The CPU first:** use the memory, scheduling, clock and SIMD-state APIs that exist (section 3.2), and the ones asked of `KRN` for 262.
- **A GPU separately:** the driver, queues, memory, synchronization and an analysis of DMA authority are 178's. No GPU driver inside the optimizer without a separate task.
- **The repository is at hand.** MIND Core's target, ABI and APIs are in this repository (`common/abi.rs`, `libmind`, `idl/`). Until the native adapter runs, a host adapter and a documented platform contract are made; a stub is marked as a stub, never reported as native integration.

## 13. Checking quality and performance

| Kind of task | Examples of a quality profile |
|---|---|
| Autoregressive language | NLL/perplexity on the same tokens; separate applied checks |
| Classification | Accuracy/F1 and errors per class |
| Regression | MAE/MSE, an agreed normalized measure |
| Embedding/retrieval | Recall@k/nDCG and ranking quality on the task |
| Vision, audio and other families | A profile of their own once their adapter exists |

- **Metrics are chosen by the task and fixed before the search.** A perplexity threshold of 1.05 may stay an example of a language profile. It does not mean "5 % less intelligence", and does not carry over to other tasks ([S1]).
- **Numerical correctness** compares inputs and outputs of single operations, blocks and the full model.
  - Tolerances are set per dtype and operation. Bit-for-bit equality between backends is not promised.
  - Gradients are checked apart. At a linear spline's kinks a convention is set, and a plain central difference is not an unconditional reference.
- **A language model** checks prefill, sequential decoding and that cached and uncached computation agree. Another adapter compares its own states and outputs, and no KV cache is forced on it.
- **What is measured:**
  - the standalone artifact's size, resident weights, working buffers, session state, peak memory, latency and throughput;
  - for language, the context and generation lengths.
  - Compressing the MLPs does not by itself reduce the embeddings or the KV cache.
- **Fair comparison:**
  - baseline and candidate run on the same hardware and a comparable backend, with the same dtypes, data, shapes and amount of work;
  - quantization experiments state which part of the gain is the representation's and which the precision's;
  - a Python baseline against a Rust candidate does not isolate the splines' effect: a Rust dense baseline is required.
- **How it is measured:**
  - warm-up, waiting for asynchronous work to finish, repeated measurements, the raw numbers and the statistics described;
  - loading, compilation, weight packing and steady state counted apart.
  - Fewer FLOPs or parameters do not replace a measurement of time (MC-12.2).

## 14. Plan and dependencies

| ID | Task | Depends on | Result / done when |
|---|---|---|---|
| T00 | Inventory of the reference and of this repository | — | Files and hashes confirmed, the list of APIs that really exist, a repeatable test status; the unknown marked |
| T01 | Specifications of the contracts and the format v0 | T00 | Block semantics, a capability matrix, the manifest, errors; no Qwen constants in the core |
| T02 | A spike of the Rust backend and of the build for the target | T01 | CPU forward and one training step; a dependency and FFI audit; an ADR of the choice |
| T03 | Port of the five representations | T01, T02 | Differential tests, tails and knots, factors and bias, forward and backward |
| T04 | Import of QSL and the general export | T01, T03 | Overlay and standalone round trips, malformed-input tests, licences and provenance |
| T05 | An independent small feed-forward adapter | T03, T04 | A full non-language task, and the general pipeline working without Qwen code |
| T06 | The first language adapter and a dense baseline | T02, T04 | A real small model; configuration, logits, tokenizer and states checked |
| T07 | Calibration and local training in Rust | T03–T06 for the supported profiles | Pairs collected, validation selection, gradients, state saved; no Python required |
| T08 | General evaluate, benchmark and compare | T05–T07 | At least two task profiles, compatible reports, a baseline on the same backend |
| T09 | A manual mixed plan and a bounded search | T07, T08 | Choice by validation and by actual measurements; the combination checked whole |
| T10 | Several replacements and adaptation to the student's activations | T07–T09 | An explicit teacher-target protocol, end-to-end control of accumulated error |
| T11 | The native MIND Core adapter, inside 262's service | T01, T02, T08; 262's `nnd`; the `KRN` memory request | Execution on MIND Core: budgets, cancellation, session isolation, failure handling |
| T12 | Optimization of confirmed bottlenecks | T08, and T11 for the target's numbers | CPU SIMD, tiling and parallelism, or an available GPU through 178; quality measured again |
| T13 | More families and representations | T08, T09 | Their own adapters and profiles; the shared components not rewritten |

- **T05 and T06 may run in parallel.**
- **T11 starts with the platform contract** before training is done, but is declared done only after a real run.
- The order of stages promises no date.

### Starting execution (2026-10-25)

The maintainer assigned NRL to the Codex session (`codex/NRL-lab`). The first tasks are [900-NRL-0001](../issues-done/900-NRL-0001-reference-inventory.done) (T00), [0002](../issues-done/900-NRL-0002-initial-contracts.done) (T01), [0003](../issues-done/900-NRL-0003-cpu-backend-spike.done) (T02), and [0004](../issues-done/900-NRL-0004-first-linear-spline-block.done) (the first linear-spline slice of T03, with a minimal block round trip). These do not complete all five representations, a model adapter or T11. The remaining T-rows will be numbered as their work starts.

## 15. Acceptance criteria

- **Numerical MVP:** the five representations in Rust, the format and its round trip, reference tests, a backend smoke test, reproducible reports. Not yet an optimized real network.
- **Multi-model research MVP:**
  - the first full language model and an independent feed-forward architecture;
  - the same shared conversion and training tools, with different metric profiles;
  - a manual choice of representations;
  - a measured result on held-out data.
  - No gain is allowed as an honest result, but is not called a speed-up.
- **Native MVP:**
  - a standalone artifact loads and runs on MIND Core without Python, PyTorch or a hidden host environment;
  - resource limits and the cross-domain contract checked.
  - A fully native cycle is claimed only when import, calibration, training and evaluation inside the OS are each shown.
- **An accepted optimization of a given model:**
  - meets the quality and resource limits fixed in advance;
  - improves the chosen target metric by full measurements;
  - every compromise published.
  - A smaller overlay, fewer parameters or a lower local error alone are not enough.
- **Skipped tests, unsupported kernels, a CPU fallback and unchecked profiles** stay visible in the reports. They are never replaced by zero errors or by the word "supported" (MC-12.1, MC-12.3).

## 16. Risks and how false results are prevented

| Risk | Measure |
|---|---|
| A Qwen-specific script rewritten instead of a platform | An early second adapter and task profile; no family constants in the core |
| Splines more complex but no more useful than plain compression | `pruned` and `svd` as controls; negative results published |
| A good local error, a bad full model | End-to-end checks after single and joint replacements |
| A speed-up of rewriting the language credited to the representation | A dense Rust baseline on the same backend |
| Too many representations and graphs at once | A narrow operator profile and an explicit backlog |
| A `no_std` crate that still does not build or run in the OS | An early target spike, a feature audit, a check of linking and running |
| Hidden network access or code execution on import | An isolated importer, data-only formats, no implicit download |
| Weights that fit, inference that runs out of memory | Scratch, KV/state, input length and the number of sessions counted |
| Checks removed for speed | A separate checked import and a fast runtime that keeps its invariants |
| Weights changed under an old session's state | Versioning, and refusal of incompatible reuse |

## 17. What to do first

1. **T00–T03, not a full rewrite of Qwen:**
   - unpack the unchanged reference into a scratch directory;
   - reproduce the tests that can run, and record the skips and versions;
   - read MIND Core's target, ABI and APIs in this repository (section 3.2).
2. **An ADR and a small Rust workspace under `nrl/`:**
   - a typed block description and a CPU profile;
   - a linear spline and one gated MLP;
   - tests against stored reference tensors.
3. **Then** the other four representations and a simple independent adapter.

The first pull request holds a working narrow numerical path, its tests and a clear list of the unknown. It does not also write a GPU driver, a universal graph compiler, a new OS compatibility layer and every network family.

## Sources

- **[S1]** `qwen-spline-lab-v0.1.0.zip` → `qwen-spline-lab/README.md`, sections 1–8: representations, tools, storage and limits.
- **[S2]** The same archive → `qwen-spline-lab/docs/TEST_STATUS.md`: the delivery's protocol of 22 October 2026, historical, not a new run.
- **[S3]** The MIND Core Constitution: the handoff read edition 1.3 (20 September 2026). This issue cites [edition 1.6](../constitution/EN/MIND_CORE_Constitution_v1.6.md), which this repository holds. It states requirements and proves no implementation.
- **[S4]** The maintainer's words in the handoff's dialogue:
  - approximation and fine-tuning are allowed;
  - separate tools and a choice of representations are needed;
  - the target OS is Rust;
  - Qwen is one example for later experiments with many networks.

## Related

- [262](262-model-runtime-on-the-machines-full-hardware.md) (the model service it runs in, and its stages 4 and 5);
- [178](178-models-on-every-accelerator.md) and [174](174-full-use-of-pc-hardware.md) (the accelerators);
- [250](250-voice-dictation.md), [251](251-model-cache-and-model-disk.md), [252](252-neural-speech-synthesis.md) (`mind::nn` and its models);
- [docs/nrl/HANDOFF_RU.md](../docs/nrl/HANDOFF_RU.md) (the original handoff).

## Initial host slice verified (2026-10-25)

On `codex/NRL-lab`, 900-NRL-0001 through 0004 are complete: the unchanged reference passed 64 CPU tests, and the first Rust block profile passed 11 host tests plus two portable release builds. Source and commands are in [NRL's README](../nrl/README.md), [reference inventory](../docs/nrl/reference-inventory.md), and the [CPU report](../nrl/reports/cpu-f64-2026-10-25.json). Code is awaiting the landing queue; these are not guarantees of the native platform.

T00 is complete. T01 has its initial block/artifact contract; full-model contracts remain. T02 has the narrow CPU spike. T03 is only dense and spline-linear so far, and the block round trip is only a small part of T04. Next are channel pruning, cubic Hermite and SVD with differential checks, then model import and the independent feed-forward adapter. APP's nnd integration stays T11. CI wiring is requested from KRN in requests-KRN.md; existing scripts do not discover this workspace automatically yet.

## Continued independently of landing (2026-10-26)

At the maintainer's instruction, NRL continues on its own branch while landing is deferred. [900-NRL-0005](../issues-done/900-NRL-0005-pruning-and-shared-channel-selection.done) adds pruned controls and shared deterministic channel selection. Its 16 host tests, both portable rlibs and three external fixture cases passed; [dated evidence](../nrl/reports/pruning-f64-2026-10-26.json). Block schema 1 replaces schema 0, with explicit refusal and regeneration rather than a compatibility reader. Cubic Hermite and SVD are next; T03, T04 and native T11 are not closed by this slice.

[900-NRL-0006](../issues-done/900-NRL-0006-cubic-hermite-block.done) adds cubic Hermite with linear tails and analytic coefficient gradients. Twenty host tests, both portable rlibs and four external fixture cases passed ([evidence](../nrl/reports/hermite-f64-2026-10-26.json)). Schema 2 replaces earlier prototypes. SVD, supported affine/bias contracts and full-model work remain; this is not B-spline or native support.

[900-NRL-0007](../issues-done/900-NRL-0007-factorized-svd-blocks.done) adds direct SVD factor execution, all factor gradients and a bounded Rust f64 initializer (dimensions <=64, <=128 sweeps; nonconvergence is refused). All five bias-free block paths passed 25 host tests, two portable rlibs and five external fixture/train/round-trip cases ([evidence](../nrl/reports/svd-f64-2026-10-26.json)). Schema 3 replaces earlier prototypes.

The next independent work is to specify and implement affine/bias behaviour and operator applicability (remaining T01/T03), then T04's QSL import and general overlay/standalone provenance contracts, and the independent feed-forward adapter. Large-model decomposition still needs an appropriate backend; the small initializer is not one. Parent 900 and the general T03 acceptance criteria remain open. Landing, KRN CI wiring and APP's future nnd are integration work, not host-development blockers.

[900-NRL-0008](../issues-done/900-NRL-0008-affine-bias-and-applicability.done) completes optional affine projection behaviour and explicit applicability for all five numerical paths: preserved gate/up/down biases, every bias/factor gradient, unsupported-contract/device refusals, and named old-schema refusals. At 52773a8e, 31 host tests, both portable rlibs and 12 differential/train/round-trip cases passed ([evidence](../nrl/reports/affine-f64-2026-10-26.json)). The original bias-free QSL oracle and new affine/rectangular PyTorch extensions are distinguished.

T03's five representations now have a checked gated SiLU CPU f64 profile, including affine biases, gradients, factors, knots/tails and block round trips. Other operator families still need their own contracts; T01's full-model contracts and T04 onward remain open. Next is bounded QSL import and general artifact/provenance structure, before the independent feed-forward adapter. No native execution or model-level quality/compression result follows from this numerical profile.

[900-NRL-0009](../issues-done/900-NRL-0009-qsl-overlay-block-bridge.done) delivers the first T04 bridge: all five QSL overlay block representations import with bounded strict JSON/F32 safetensors, original checksums/artifact ID, exact parameter widening, preserved source/licence records and explicit unverified base identity. Unknown retained-channel indices remain unrecorded. Schema 5 and the separately scoped `nrl.block-set/0` package are documented; this is a set of standalone numerical blocks, not a whole runnable model.

At 9a7374cf, 37 host tests, two portable rlibs, the 12-case numerical demonstration, five external QSL block outputs and CLI import/reload passed ([evidence](../nrl/reports/qsl-block-bridge-2026-10-26.json)). T04 is still open: general model/graph manifests, base identity verification, QSL standalone-model import and complete overlay/standalone model export. The independent feed-forward adapter follows those contracts. Landing remains asynchronous and does not block NRL host development.

[900-NRL-0010](../issues-done/900-NRL-0010-model-graph-and-packages.done) adds full-model contracts and overlay/standalone semantics for a bounded stateless feed-forward profile: explicit affine/gated/SiLU/ReLU/add graph, verified tied-object inventory, exact base identity and preserved source/licence/notices/history metadata. Resolving an overlay produces a complete standalone model that runs after the base and overlay files are removed. This is the first independent `nrl.feed-forward` graph adapter, including an ordinary affine/ReLU/affine numerical oracle.

At fed14350, **44 host tests**, both portable rlibs, the existing numerical/QSL regressions and complete model CLI cycle passed; all six new fixture files regenerate byte-identically ([evidence](../nrl/reports/model-graph-2026-10-26.json)). T01/T04 now have checked complete feed-forward contracts/packages. The graph currently executes forward only: next is T05's explicit dataset/task/evaluation and full-graph training pipeline. T04 remains open for QSL standalone language models and original language-model/tokenizer identity; T06 and native T11 remain separate. Synthetic complete-model numerics do not establish pretrained-model quality, optimization or native guarantees. Landing remains deferred and does not block the next host task.

[900-NRL-0011](../issues-done/900-NRL-0011-feed-forward-training-and-evaluation.done) delivers T05's first complete independent regression task: full-graph backward and tied-object SGD, explicit related-group-aware train/validation/test data, validation-only checkpoint selection including epoch zero, and held-out baseline/selected metrics. New versioned experiment schemas preserve source/licence/notices and existing model history, bind a separate SGD training record, and support checking and complete replay from saved inputs without Python. Current model/graph/block formats are unchanged.

At f76c8376, **52 host tests**, both portable rlibs, all earlier numerical/QSL/package regressions and the full train/check/replay CLI cycle passed ([evidence](../nrl/reports/regression-training-2026-10-26.json)). Full-graph gradients and a single step match PyTorch; all 271 trainable values and nine inputs have independent central differences. All 129 train/validation history rows and selected parameters match the external regression oracle. Validation chooses epoch 91 of 128; changes to test targets cannot affect parameters or selection. The [complete saved example](../nrl/reports/regression-example-2026-10-26/report.json) includes both models, data/config and the actual Rust training record and can be replayed into a new directory.

T05 has its first complete non-language regression profile, with initial general host training/evaluation contracts from T07/T08. This does not close T07/T08: local calibration/distillation, optimizer resume, task-general evaluation/benchmark/compare, and a second task profile remain. Next independent work is T06's real language adapter/dense baseline and its remaining T04 import/identity checks, then calibration and model-level comparisons. T09/T10 plans/search and student adaptation, T11 native integration, T12 measured optimization and T13 expansion remain open. The regression fixture is synthetic, not a pretrained-model or compression result. Landing and KRN CI wiring stay asynchronous integration work.

900-NRL-0012 completes T06 for a real trained TinyStories-260K/custom-tokenizer profile: full scalar f64 decoder, GQA/RoPE/RMSNorm/tied head, byte-BPE, model-bound KV state and bounded standalone packages. **57 host tests**, both portable rlibs, independent actual-weight PyTorch f64/original C float32/tokenizer checks and CLI import/check/probe pass ([dense baseline](../nrl/reports/decoder-baseline-2026-10-26.json)). Original dense storage is retained; no held-out language or compression result is implied. T07 calibration/checkpoints and T08–T10 evaluation/plans/adaptation continue autonomously at the maintainer's explicit request.

900-NRL-0013 completes T07 for the bounded trained decoder: split-aware original-teacher train/validation pairs, four Rust compressed initializers, validation-only local SGD and exact serialized optimizer/data/order/current/best continuation. **62 host tests**, both portable rlibs and calibration/train/resume/export CLI cycles pass ([evidence](../nrl/reports/local-training-2026-10-26.json)); the [saved example](../nrl/reports/local-example-2026-10-26/report.json) includes actual calibration and partial/complete checkpoints. Test activations do not initialize/train/select. The six T09 plans, quality/package budgets and T10 protocol/rounds were frozen before T08 test evaluation. T08–T10 continue next; local errors do not establish model quality or compression.

T08 is complete on the NRL branch (900-NRL-0014): 65 host tests, both portable rlibs, two independent task metrics and compatible repeated timing reports. [Evidence](../nrl/reports/evaluation-2026-10-26.json). Original dense storage and whole-process RSS are reported honestly; the current linear student loses language validation quality. T09/T10 continue using the unchanged pre-frozen study.
