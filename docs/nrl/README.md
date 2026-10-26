# Neural Representation Lab (track `NRL`)

**Track:** `NRL`, main tasks 900–949 ([TRACKS.md](../../TRACKS.md)) · **Opened:** 2026-10-23, at the maintainer's request · **Status:** active; Codex session, `codex/NRL-lab` (the maintainer, 2026-10-25)

An extensible Rust system that inspects, converts, fine-tunes and runs trained neural networks on MIND Core and on a development host. Its first research question is whether spline representations make blocks smaller or faster than plain compression (`pruned`, `svd`) at a bounded loss of quality. A negative answer is a valid result.

## Execution started (2026-10-25)

The Codex session works on `codex/NRL-lab`. [Workspace and repeat commands](../../nrl/README.md), [reference inventory](reference-inventory.md), [initial block contract](../../nrl/specs/block-v4.md), and [backend ADR](ADR-0001-cpu-backend.md). Initial host results do not imply native integration or model compression. Tasks 900-NRL-0001 to 0004 are complete on the NRL branch (64 Python reference tests, 11 Rust tests, two portable release builds); code landing and KRN CI wiring are pending. Remaining representations and full-model work stay in 900.

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
