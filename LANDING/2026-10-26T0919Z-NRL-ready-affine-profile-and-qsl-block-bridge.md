# Ready: affine NRL profile and QSL overlay block bridge

**From:** NRL · **To:** LANDING · **Written:** 2026-10-26 09:19 UTC · **Concerns:** 900-NRL-0001 through 0009; parent 900/T04 remain open

**Ready:** `codex/NRL-lab` bd14e5be56fe568195e7b34d4f17ab1be86154e1

This replaces the unanswered `LANDING/2026-10-26T0756Z-NRL-ready-five-bias-free-block-paths.md` for the same branch. The maintainer asked for autonomous NRL development and deferred landing for a couple of days; no immediate gate/response is requested and host work continues independently.

New completed slices:

- 900-NRL-0008: optional gate/up/down output biases and gradients across all five representations; bias-preserving pruning/spline/SVD conversion, explicit semantic/device applicability and named format refusals. Its recorded run passed 31 host tests.
- 900-NRL-0009: bounded strict JSON/F32 safetensors, all-five QSL overlay block import, source/patch hashes and original artifact-ID checks, explicit unrecorded channel mappings and provenance-preserving standalone numerical block sets. Python is an external fixture generator only; the CLI importer/loader has no Python/network path.

Current block schema is 5, replacing NRL schemas 0–4 with explicit refusals, without compatibility readers. `nrl.block-set/0` has scope `standalone-numerical-blocks`, mandatory unverified base identity and caller-provided weight licence. It is not a runnable whole-model artifact; QSL standalone-model import and general graph/model export remain T04 work. Native nnd and GPU work remain separate. Existing project `libmind/src/sha256.rs` is reused unchanged by the host bridge; no dependency or edits in another track's source were added.

Validation on NRL's own branch at 9a7374cf after fast-test staging 278da0b8: **37 Rust host tests**, portable release rlibs for x86_64-unknown-none and aarch64-unknown-none-softfloat, **12** external block forward/backward/SGD/exact-round-trip cases, and the five-representation QSL import/package/CLI cycle passed. Tests include every bias/factor's independent numerical gradient, malformed hashes/JSON/tensors/shapes/offsets/paths, explicit source/size records and no-overwrite. Original-writer regeneration matched tensor bytes, external outputs and core manifest (recorded environment excluded from cross-host comparison). No build/test ran on fast-test.

Evidence: `nrl/reports/affine-f64-2026-10-26.json`, `affine-check-2026-10-26.txt`, `qsl-block-bridge-2026-10-26.json`, `qsl-check-2026-10-26.txt`. The final commit only records results and closes scoped tasks; numerical/import code, fixture bytes and commands are identical to the tested source commit.

Please run `bash nrl/tools/check.sh` explicitly in the eventual candidate checks until KRN CI wiring lands (the existing request is in `issues/requests-KRN.md` on main fb821bbd). THIRD_PARTY.md is image-copied, so the code package requires the full landing gate. Portable rlibs are not linked/native runs; no real-model compression, quality or speed benefit is claimed.
