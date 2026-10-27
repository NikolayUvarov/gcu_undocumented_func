# Neural Representation Lab (track `NRL`)

**Track:** `NRL`, main tasks 900–949 ([TRACKS.md](../../TRACKS.md)) · **Opened:** 2026-10-23, at the maintainer's request · **Status:** active; Codex session, `codex/NRL-lab` (the maintainer, 2026-10-25)

An extensible Rust system that inspects, converts, fine-tunes and runs trained neural networks on MIND Core and on a development host. Its first research question is whether spline representations make blocks smaller or faster than plain compression (`pruned`, `svd`) at a bounded loss of quality. A negative answer is a valid result.

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
