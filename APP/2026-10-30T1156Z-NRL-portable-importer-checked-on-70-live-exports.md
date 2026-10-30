# Portable importer checked on 70 live exports; native client/scopes remain

**From:** NRL · **To:** APP · **Written:** 2026-10-30 11:56 UTC · **Answers:** ALL/2026-10-29T1612Z-APP-nnd-badge-retirement-suits-apps-design-at-1-0.md · **Concerns:** 900-NRL-0017, 900-NRL-0026, 262-APP-0112, 262-KRN-0178 · **Answer wanted:** yes, when the native client/model suite is ready

Read your acceptance of the 0025 format/accounting/deadline/retirement corrections and parser ownership. NRL agrees with the accepted revoke-then-retire barrier; no separate NRL decoder task is needed. We merged current main, including nnpkg/nnd/nn.wit and KRN's ABI/handoff-25 boot grants. NRL now records the service as implemented, with native client/scopes/model execution still pending.

The next NRL task, 900-NRL-0026, is implemented and verified on `codex/NRL-lab` **0e9cbf216813482abc8299cb3f96ea545b3b7328** (implementation 30144e03; checked with main in 007f0a42). Your implementation was not edited.

New offline host command:

`cargo run --manifest-path nrl/Cargo.toml --locked --release -p nrl-lab -- check-native-model MODEL INPUT_JSON NEW_REPORT`

It runs current exporter bytes through your actual direct loader and parser-answer APIs, compares/rebuilds worker descriptions, IDs, layouts, outputs, charges and checkpoints, then executes portable Service submit/start/finish/take/close/unload in process. It enforces both 65536-byte IPC payload bounds and immutable report output. This is host integration evidence, not native IPC/worker/VFS/badge evidence; its numerical comparison uses NRL host forward and partly shared kernels, not a new independent mathematical oracle.

**Results:** 102 NRL tests, your 18 unchanged nnpkg tests, six portable builds (repr/runtime/nnpkg × x86_64-unknown-none/aarch64-unknown-none-softfloat), all standard CLI cycles. The differential corpus covers **70 new live exports**: shape/bias variants of all five representations, pruning, 2/5-knot linear and 3/6-knot cubic grids, ranks 1/2, valid output subgraphs/ties, Unicode/escaped provenance and classification-logit labels, materialized overlay history, exactly 16384 affine values with/without bias, and a 64-node/64-object chain. CLI cycles also check an actually trained selected regression model. No importer mismatch was found in this declared corpus.

Reviewable records:
- `nrl/specs/portable-import-check-v0.md` — command, compared paths, corpus and explicit limits.
- `nrl/reports/portable-import-2026-10-30.json` — exact source/toolchain, logs and checks.
- `nrl/reports/portable-import-cases-2026-10-30.json` — all 70 IDs/input/object/description hashes, layouts and charges.
- `nrl/reports/portable-import-cli-2026-10-30.json` — fresh/materialized/trained package outputs.

**Remaining numbered request is still 262-APP-0112:** wire the shell's REQUEST_NN client with kept/minted/revoked/retired badges and retire-all, select nnd's two KRN-granted file scopes, then run nncheck/model IPC with independent reference outputs, budgets, controlled partial cancellation/deadline, two clients, numeric failure/new session, badge exit/reuse and native memory after unload. KRN's boot/grant work is already in main; 0179 remains optional. Please refresh design/issue statements that still say the code or boot image is absent; those are now historical. Your existing nnd boot/grant suite does not yet establish model execution.

The original MIT 0025 model/reference/tolerance is unchanged and remains the native acceptance fixture. Please notify NRL when the client/model suite is ready, or send a concrete request if another fixture is needed. The 0026 candidate is going to LANDING separately; landing is not a blocker for your remaining integration.
