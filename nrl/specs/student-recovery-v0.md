# Student-activation recovery v0

Serves 900 T10; MC-5.1, MC-8.5, MC-9.1, MC-12.1–12.3, MC-12.9. Initial interfaces: `nrl.recovery-pairs/0`, `nrl.recovery-record/0`, `nrl.recovery-envelope/0`, `nrl.recovery-summary/0`.

The exact unpatched teacher, corpus and pre-frozen two-layer start plan are required. The input student is verified against deterministic reconstruction of that plan. The example uses `mixed-linear-pruned`, three recovery rounds, 64 train rows and learning rate 0.001, fixed before candidate test evaluation.

For `frozen-teacher-on-student-input/0`, freeze the complete current student for one round. In corpus order, execute train-only token prefixes with fresh per-document causal caches; the row cap selects prefixes explicitly. Save normalized FFN inputs at every replaced layer. Evaluate the original frozen teacher FFN on those same student inputs to obtain targets. These targets are neither old teacher-trajectory outputs nor teacher logits. No teacher parameter changes. Downstream inputs reflect the actual preceding student replacements.

Each replacement takes one analytic full-batch local mean-square-error SGD step on those pairs. All gradients use the same pre-round trajectory and parameters; install all updated blocks together. Embeddings, attention, normalization, output head and external shapes stay unchanged. Fixed spline grids and source-channel mappings stay fixed. No end-to-end decoder/NLL backward pass, optimizer momentum, random sampling or hidden shuffle is implemented or claimed.

The matched control, `frozen-teacher-on-original-input/0`, uses the original teacher trajectory and the same initial patches, row order, round count, learning rate, local step algorithm and model-level selection. Its downstream inputs do not adapt to the evolving student. Actual pairs for both arms identify source teacher, current student, input-producing model, corpus, train split, documents/groups/tokens, layer, inputs and targets; their canonical hashes bind them into the run history.

Round zero and every completed round record full-model validation mean NLL and whole-logit MSE/maximum error against the original teacher over identical validation tokens. Each arm keeps the earliest minimum validation NLL, including round zero. Only after both arms select does held-out test evaluation run. A separate deployment recommendation uses validation quality, actual package ratio and observed nonoverlapping timing under the T09 rule; it retains dense if any condition fails. A recovery output is a research artifact even when dense is recommended.

The canonical deterministic run record binds settings, corpus, source/start identities, stateless optimizer/order, pairs, local errors and whole-model validation histories/selection. Final models append its hash as source provenance; the record stores pre-provenance model IDs to avoid a hash cycle. An envelope checks integrity, not authenticity. Same-profile replay recomputes initialization, trajectories, targets, updates, selection, model IDs and task metrics and compares saved pairs/models/reports. Timing/RSS are new observations and are not expected to repeat. Existing reports must be internally consistent with their saved summary and recommendation. An incomplete output without the final `complete.json` marker is refused.

Independent tools:

```text
recover-plan TEACHER INITIAL_STUDENT CORPUS STUDY NEW_BUNDLE
check-recovery TEACHER SAVED_BUNDLE
replay-recovery TEACHER SAVED_BUNDLE NEW_BUNDLE
```

`check-recovery` performs recomputation without overwriting the bundle; replay additionally saves a new run with its own measurements. Each directory/file must be new; interruption can leave an incomplete directory. Pair files and records are bounded to the existing 16 MiB research JSON limit, 2–3 replacements, at most 256 rows and eight rounds per arm. These host bounds do not implement native cancellation, hardware isolation or per-process quotas. Original dense storage remains retained; fewer effective parameters do not establish memory/disk savings. The miniature corpus and scalar host measurements do not establish representative language quality, statistical significance or target performance.
