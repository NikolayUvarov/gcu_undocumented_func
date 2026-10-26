# 900-NRL-0011 — Full-graph feed-forward training and split-aware regression experiments

**Type:** research and implementation, `NRL` · **Owner:** Codex session (`codex/NRL-lab`) · **Priority:** P1 · **Status:** in progress · **Blocked by:** — (0001 through 0010 are on this branch) · **Roadmap:** G; issue 900 T05, initial host T07/T08 contracts · **Constitution:** MC-5.1, MC-8.5, MC-9.1, MC-11.2, MC-11.3, MC-11.8, MC-12.1, MC-12.2, MC-12.3, MC-12.4

## Problem

The independent feed-forward adapter runs complete graphs but has no graph backward pass or complete dataset/task experiment. A block SGD check is not full-model training or a held-out evaluation. T05 needs an independent non-language pipeline with explicit data separation, selection rules, provenance and repeatable results.

## Plan

1. Add full-graph vector-Jacobian products for every supported operator and all five gated representations. Accumulate branches and tied-object gradients; update each distinct object once.
2. Enforce separate logical training-buffer and work budgets, finite gradients and transactional parameter updates. Keep forward and native contracts unchanged.
3. Add versioned bounded regression data with explicit train/validation/test membership, stable sample/group IDs, source declarations and cross-split duplicate/group checks before computation.
4. Train the complete supported graph with deterministic full-batch SGD, evaluate train/validation each epoch and select the earliest minimum-validation-MSE checkpoint, including the initial model. Evaluate test only after selection.
5. Save a new experiment directory containing input model, data/config, loss history, a content-bound training record and selected standalone model. Preserve source declarations and existing history; describe SGD provenance separately from manual overlay replacements. No exact resume or useful-compression claim.
6. Check graph gradients and SGD against independent PyTorch, every parameter/input by finite differences, split/selection independence, repeatability, saved package integrity and CLI cycle.

## Acceptance criteria

- [ ] Every supported graph operator and all five gated representations have independently checked full-graph gradients; sharing/branch accumulation and single-object updates are correct.
- [ ] Invalid gradients, nonfinite updates and input/training/work budgets fail without mutating the initial model.
- [ ] Data are separated before computation; IDs, related groups, exact duplicate inputs, source metadata, shapes and limits are checked.
- [ ] Only train drives gradients and only validation chooses the checkpoint; test-target changes cannot affect selected parameters or epoch.
- [ ] A repeatable ordinary feed-forward regression experiment reports baseline and selected metrics, selected epoch, exact data/config/model identities, actual work and limitations.
- [ ] Exported selected standalone models preserve source licences/notices and existing provenance, bind a versioned training record and run without the experiment's other files; outputs are never overwritten.
- [ ] Host numerical/package/CLI checks and both portable release rlibs pass with dated evidence. Classification, language, local calibration/distillation, optimizer resume, search, benchmarking and native work remain explicit.

## Related

- [900](900-neural-representation-lab.md), T05 and sections 8, 13–15.
- [0010](../issues-done/900-NRL-0010-model-graph-and-packages.done).
- [NRL](../nrl/README.md).
