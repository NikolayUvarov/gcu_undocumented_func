# Full-graph regression experiments v0

**Edition:** 0 (2026-10-26) · **Task:** [900-NRL-0011](../../issues/900-NRL-0011-feed-forward-training-and-evaluation.md) · **Scope:** independent stateless feed-forward regression on the host scalar CPU f64 profile. This introduces experiment/data/run/record/report schemas; `nrl.model/0`, `nrl.graph/0` and `nrl.block/5` are unchanged.

## Differentiation and parameter updates

`Model::backward(input, output_gradient, max_training_values)` computes a vector-Jacobian product for the complete supported graph. The objective derivative is supplied explicitly. Affine and all five gated representations use their checked analytic backward passes. SiLU uses its analytic derivative; ReLU uses zero at exactly zero. Add sends its adjoint to both input edges, including two edges to the same value. Branch derivatives accumulate. A shared object ID owns one gradient accumulator across every use. Equal bytes under different IDs are still different parameters.

`sgd_parameters` returns replacement parameter data, updating each distinct object once by `w - learning_rate * gradient`. Invented/missing gradients, kinds/shapes/ranks, nonfinite values/rates and overflow are errors. The original loaded model is immutable; a failed update cannot partially mutate it. Returning parameter data alone is not a provenance event or model package export.

The backward call validates its logical budget before retaining a forward tape or allocating adjoints. It accounts for twice the retained graph activations plus the distinct trainable parameter gradients. The declared positive limit is at most 1048576 values. This count excludes per-object scratch, transient derivative/output copies, model snapshots, parsed data, JSON/source bytes, history and host containers. It is not an RSS limit or native memory capability. Existing model batch and activation limits apply independently.

## Dataset `nrl.regression-data/0`

A dataset declares input/output widths, one source record, and samples with `id`, `group`, `split`, `input` and `target`. Source identifiers, optional hashes, licence declarations and notices use the model contract's bounds; they are retained, not independently authenticated or legally verified. Only `train`, `validation` and `test` splits are accepted and all must be nonempty.

The complete dataset is validated before any activation is computed. Sample IDs are unique. Sample/group IDs contain 1..128 ASCII alphanumeric or `_.-` characters. A related group cannot cross splits. Exact equal input rows cannot cross splits, regardless of targets, with positive/negative zero treated as equal. Within-split repetitions are permitted and contribute their supplied weight through row multiplicity. Approximate duplicates and undeclared relationships cannot be inferred. Split membership and group declarations are caller supplied; this command does not discover a statistically independent split automatically.

Widths must match the model; input/target lengths and finite values are checked. Limits are 4096 samples, 4096 per width and 262144 total input/target values. Every split must also fit the model's batch/activation limits. Regression models declare `regression-vector/0`; classification task labels alone do not enable this trainer.

The dataset ID is SHA-256 of typed normalized compact JSON with recursively sorted object keys and retained array order, including targets, source metadata, group and split declarations. Row order is part of identity. Floating-point parse/serialization uses the pinned `serde_json` float_roundtrip profile. Identity does not establish authenticity. Altering test targets changes provenance identity and test metrics but cannot select different parameters or an epoch.

## Run `nrl.regression-run/0`

The only optimizer is `full-batch-sgd/0`, with a positive finite fixed learning rate, 1..256 epochs, the training-value limit and `max_forward_rows` at most 65536. Selection is `validation-mse-earliest/0`. The run also supplies an output source/licence/notice declaration; its hash must be absent because the tool fills a derived training-record hash. Its identifier is at most 1983 bytes, leaving space for `#` plus that hash.

There is no RNG, shuffle, dropout, momentum, weight decay or minibatching. Training rows retain their supplied order. The loss is mean squared error over all train output values. All object weights, factors, spline values and declared biases train; fixed spline grid bounds and retained-channel mappings do not. Graph gradients include upstream effects from later nodes and every shared use.

Epoch zero is evaluated on train and validation. Each epoch computes train gradients, updates all parameters transactionally, then evaluates train and validation. A strictly smaller validation MSE selects that checkpoint; ties keep the earlier one. All configured epochs complete; selecting epoch zero is valid. Test metrics for the initial and selected models are computed only after the choice is complete. No test metric chooses a step count, parameter or checkpoint.

Before computing activations, the run checks training/batch limits and the forward-row work budget:

`train_rows + validation_rows + epochs * (2*train_rows + validation_rows) + 2*test_rows`.

Each epoch includes one forward tape recomputation for backward and one evaluation of updated train/validation outputs. This counts graph forward rows, not machine instructions, backward arithmetic, seconds or construction/serialization work. There is no native deadline, cancellation or hard-real-time claim.

## Metrics and reporting

MSE, MAE, maximum absolute error and target mean energy are reported for each split. NMSE is MSE divided by `max(target_mean_energy, 1e-12)`; a flag records target energy at or below that epsilon. Every reported numeric value must be finite. Target normalization uses the task's own output, not an unrelated residual. No latency, RSS, speedup or compression claim is derived from these metrics.

`nrl.training-record/0` binds initial model ID, dataset/run IDs, epochs, selected epoch, actual forward-row count, before/after object hashes, all train/validation history and baseline/selected split metrics. It records numerical policy, fixed order/no RNG, optimizer/resume limits, implementation version and host OS/architecture. A dated validation report supplies the exact tested source commit/toolchain; this record is not a full hardware benchmark description.

`nrl.regression-report/0` binds that record's exact saved bytes and both model IDs, repeats the selected epoch/work and baseline/selected metrics, and lists limitations. There are no timestamps in computational artifacts. On the same recorded executable/host configuration, complete replays produce identical records/model identities. No bit identity is promised across other backends, platforms or toolchains.

## Saved experiment and provenance

A new experiment directory contains:

- `initial-model/`: the complete original standalone model;
- `selected-model/`: the selected standalone model;
- `data.json`, `run.json`: normalized complete inputs for replay;
- `training-record.json`: versioned training provenance and metrics;
- `report.json`: the final identity/metric report.

The trainer accepts a standalone input; resolve an overlay first. Selected models preserve graph, aliasing, execution profile, existing history and all input source declarations. Dataset source metadata and the output source declaration are appended when not identical. The output source identifier ends with the exact training-record hash and its hash field binds the record's saved bytes. The record binds before/after object hashes, without a cycle through the final model ID. Existing `nrl.model/0` manual-replacement history is preserved; SGD is recorded by the new training schema, never mislabeled as a manual replacement. The model can run after all other experiment files are removed; the missing record then remains a content-bound provenance reference, not a runtime dependency.

`train-regression` executes and writes a new experiment; `check-experiment` verifies input/package/record/report identities, metadata/history/source preservation and the checkpoint-selection rule, and recomputes initial/selected split metrics. Checking does not retrain intermediate checkpoints. `replay-regression` checks the source experiment, then restarts its entire computation from the saved initial model/data/config into a new directory. This is not an optimizer checkpoint/resume format. Cross-environment replays may have different recorded identities.

JSON is bounded at 16 MiB per file and rejects duplicate keys, excessive nesting, unknown fields and unknown schemas. Existing model package limits and regular-file/symlink checks apply. Files and directories are fixed names; no source URI is followed and no package code runs. All outputs require new paths. A failed write can leave an incomplete new directory; no existing data is deleted. Model packages/data/history add host storage and memory beyond the logical training budget.

## Evidence and remaining scope

External fixtures use the unchanged verified QSL gated-block implementation and explicitly composed PyTorch graphs for a full VJP and single SGD step, plus an ordinary affine/ReLU/affine regression task with disjoint declared groups. Every graph parameter/input also has an independent central-difference check. Test-target independence, nonfinal validation selection, earliest ties, zero-energy metrics, split refusals, replay and standalone independence are checked separately.

This is T05's first complete non-language regression profile and initial full-graph host training/evaluation machinery. It is not a pretrained-model experiment, a representation comparison or evidence that splines improve anything. Classification, language/stateful graphs, local calibration/distillation, resumable optimizers, mixed-plan/search tools, benchmark/comparison protocols, measured optimization and native nnd remain separate tasks under 900.
