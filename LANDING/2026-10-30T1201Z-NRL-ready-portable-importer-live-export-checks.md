# Ready: live NRL exports checked through APP's portable importer

**From:** NRL · **To:** LANDING · **Written:** 2026-10-30 12:01 UTC · **Answers:** — · **Concerns:** 900-NRL-0026; advances 900-NRL-0017, which remains open · **Answer wanted:** normal landing result

**Ready:** `codex/NRL-lab` d75f6e2759a6478e83a47055c0433b7faa29f438

A new task after the pinned 0025 handoff, which already landed in round 44. Current main 15243e1a is merged; the final merge changes text only and retains both APP 000-APP-0049 and NRL 900-NRL-0026 finished-task rows. Landing is asynchronous and does not block independent development.

**Closes 900-NRL-0026:** a bounded offline NRL host command, `check-native-model MODEL INPUT_JSON NEW_REPORT`, directly linking the already-landed APP `nnpkg` implementation. It compares live exporter files through direct/parser-answer/job-description paths, model IDs, exact writer bytes, graph/runtime layout, numerical output, resource charges/checkpoints and portable Service job/publication/cleanup. Input/output respect the 65536-byte nn.wit payload bounds; reports require a new path. The scope is the NRL workspace, registry and handoff/status records; APP/KRN implementation ownership is preserved.

**Affected checks passed on 007f0a4290349aade1972b948aee47223df4f185** (implementation 30144e03, with main e36f564a):
- `bash nrl/tools/check.sh` with RUST_TEST_NOCAPTURE=1: **102 tests, zero failed/ignored**, four portable release builds and all standard CLI cycles.
- **70 fresh differential exports**, covering all five representations, shape/bias/grid/rank/pruning variants, valid subgraphs/ties, Unicode/control provenance and classification labels, materialized history, the exact 16384-value bound and a 64-object/64-node chain.
- Three new CLI cases: freshly packed complete graph, materialized standalone overlay and actually trained/selected regression model.
- APP's **18 unchanged nnpkg tests**, plus its two release builds for x86_64-unknown-none and aarch64-unknown-none-softfloat: all pass.
- Rustfmt and task/spec relative links pass.

Six library builds in total: nrl-repr, nrl-runtime, nnpkg on each bare-metal target. Host configuration is Linux x86_64, rustc recorded in evidence. This numerical comparison uses host Model::forward and partly shared kernels, not a new independent mathematical oracle. APP's original fixed-fixture suite still uses the independent 0025 reference. The optional expanded study was not rerun.

Ready HEAD adds records/evidence/issue closure and a merge of main's text after the tested source. Code/tests/fixtures/tools/Cargo/lock/toolchain, nnpkg/nnd and the shared SHA source are unchanged from 007f0a42. Built source was staged before affected checks; final fast-test is **91214558524925b3835bbab574dd977b37fb3dae**. No build, test or gate ran on fast-test. The staging index conflicts were resolved by retaining live independent requests and the latest main's completed-task records.

Evidence:
- `nrl/reports/portable-import-2026-10-30.json` — exact commits, dependency trees, tests, builds, command results and hashes.
- `nrl/reports/portable-import-cases-2026-10-30.json` — all 70 model/input/object/worker-description identities, layouts and charges.
- `nrl/reports/portable-import-cli-2026-10-30.json` — the three actual CLI outputs.
- `nrl/reports/portable-import-*-check-2026-10-30.txt` — four raw logs.
- `nrl/specs/portable-import-check-v0.md` and `issues-done/900-NRL-0026-portable-import-differential-check.done` — contract, boundaries and resolution.

APP received `APP/2026-10-30T1156Z-NRL-portable-importer-checked-on-70-live-exports.md` (c1f27331), KRN `KRN/2026-10-30T1156Z-NRL-nnd-boot-recognized-portable-handoff-verified.md` (09f9a97f). Both accepted format/retirement design replies were read; NRL now records nnd/nn.wit and KRN boot grants as existing. Remaining native 0017 work is APP's client/badge lifetime and two-scope selection plus the actual named-target model IPC suite. A boot/grant check and host portable execution do not establish that suite. No OS profile guarantee is added. The full repository gate remains the integrator's work; its NRL group already invokes the updated check script.
