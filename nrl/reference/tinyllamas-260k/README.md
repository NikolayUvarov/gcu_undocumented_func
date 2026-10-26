# Pinned trained TinyStories-260K reference

Source: https://huggingface.co/karpathy/tinyllamas/tree/0bd21da7698eaf29a0d7de3992de8a46ef624add/stories260K (publisher declares MIT).

The real trained checkpoint has dimension 64, hidden width 172, five layers, eight query heads, four KV heads, vocabulary/context 512 and a tied embedding/output matrix. `source.json` pins every upstream asset. `decoder-source.json` is NRL's import declaration, pinning exact model/tokenizer input hashes. `upstream-readme.md` is kept unchanged. The `.pt` pickle is not included or executed. `tok512.model` is a data-only custom SentencePiece reference; the Rust runtime uses the explicitly declared byte-BPE `tok512.bin` semantics and does not claim arbitrary SentencePiece normalization.

`reference-run.c` is unchanged primary-reference test code from llama2.c commit 350e04fe35433e6d2941dce5a1f53308f87058eb, with its MIT notice in `LICENSE`. The optional generator verifies its hash, includes it in a temporary test harness with the necessary standard headers and compiles it for float32/tokenizer goldens. It is not built by normal Rust tests or the OS. The trained arrays are separately evaluated by vectorized PyTorch in float64. Rust checks use the committed offline oracle and need no C, Python or network.

```
/path/to/reference-venv/bin/python nrl/tools/generate_decoder_fixtures.py --output nrl/tests/fixtures/decoder-v0.json --check
```

The trained checkpoint is a small English-story model, not a general assistant; the tests establish the stated configuration's numerics/format/tokenizer/state behaviour, not general language quality or useful compression.
