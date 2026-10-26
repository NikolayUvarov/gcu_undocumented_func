# End-to-end bounded embedding training v0

900 T07/T13. The existing `nrl.embedding/0` numerical package is unchanged: trainable table and checked projection parameters, arithmetic mean, L2 normalization with rejection at norm <= 1e-12. `backward_tokens` applies `(dy-y*dot(dy,y))/norm`, the existing graph VJP, then distributes mean adjoints across lookup occurrences, accumulating repeated token IDs. Vocabulary and discrete tokenization have no gradients; fixed spline grids/channel mappings remain fixed.

`nrl.embedding-training-config/0` uses bounded full-batch AdamW, temperature 0.01..1, an explicit graph training-value budget and at most 256 steps. The graded contrastive objective is mean cross entropy over train queries: document logits are cosine/temperature, target mass is proportional to `2^grade-1`. Every document in the fixed shared index participates, including as a negative. **Only query splits are held out**: this is a transductive document index, not a claim of unseen-document generalization. Validation graded contrastive loss chooses the earliest minimum, including step zero; test queries never supply fitting or selection targets.

Checksummed `nrl.embedding-training-checkpoint/0` contains the initial data-only definition, full dataset/configuration, source model ID, current/best named parameters, both AdamW moments, step and complete train/validation history. Loading verifies identity, shape, finite parameters/moments, selection and recomputed current/best losses. Serialized intermediate states continue exactly. Integrity hashes are not authenticity. Native OS inference/training and representative semantic quality remain unsupported.

The original `embedding-retrieval/0` evaluation deliberately freezes table weights for projection-only comparisons. A **new distinct task**, `embedding-trained-retrieval/0`, compares trained tables only with the exact same ordered vocabulary, lookup/output geometry, token limit and preprocessing/pooling/normalization contracts. Existing evaluation reports and input-content checks are unchanged; reports from the two tasks cannot be compared as one experiment. Generic metric, benchmark and report machinery is reused. Lookup weights remain part of each full model identity.

CLI:

- `train-embedding ENCODER DATA CONFIG NEW_DIRECTORY`: train, checkpoint, standalone selected model, six train/validation/test retrieval reports and summary.
- `resume-embedding CHECKPOINT NEW_CHECKPOINT UNTIL`; `export-trained-embedding CHECKPOINT NEW_MODEL`.
- `evaluate-trained-retrieval BASELINE ENCODER DATA SPLIT NEW_REPORT`; `benchmark-trained-retrieval BASELINE ENCODER DATA SPLIT MEASUREMENT NEW_REPORT`.

PyTorch 2.10.0 float64 autograd independently checks the complete lookup/projection/L2 gradient with repeated tokens, all graded contrastive gradients, two AdamW steps and validation losses. Rust checks exact interrupted continuation, standalone reload, explicit profile separation, invalid adjoints/budgets and unchanged selection when only test queries change. The original synthetic fixture remains numerical evidence, not a representative retrieval benchmark.
