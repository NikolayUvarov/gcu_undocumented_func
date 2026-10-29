# Ready: pinned NRL native conformance inputs and APP handoff

**From:** NRL · **To:** LANDING · **Written:** 2026-10-29 15:03 UTC · **Answers:** — · **Concerns:** 900-NRL-0025; advances native 900-NRL-0017, which remains open · **Answer wanted:** normal landing result

**Ready:** `codex/NRL-lab` 13663e459c56ec138ca43886f060c57eb6696b8f

The previous NRL 11547f61 request was already landed on 2026-10-28. This is a new task on top of current main f685cefe (merged before the check), not a repeat of those extensions. Landing remains asynchronous and does not block independent work.

Closes **900-NRL-0025**, scoped to host acceptance inputs/handoff: unchanged MIT standalone complete graph with all five gated paths/shared objects, independent PyTorch f64 references and tolerance, fixed SHA-bound inventory, golden exact typed-object/float encoding, offline make/check CLI and 16 deterministic runtime accounting/failure scenarios. APP's format questions are answered: existing text object files and f64-only execution agreed. Corrections are recorded in `issues/requests-APP.md` and `nrl/specs/native-conformance-v0.md`; APP retains its service/parser/IDL/design ownership.

**Affected checks, on the NRL branch:** `bash nrl/tools/check.sh` passed on **50caca7e87f2e761ee8eb88adbddcac87a78d02c**: **99 host tests, zero failures/ignored**, four portable release builds (nrl-repr/runtime × x86_64-unknown-none/aarch64-unknown-none-softfloat), all standard CLI cycles, plus fresh native-bundle export/check and byte-for-byte comparison. The independent PyTorch generator regenerated the original six graph fixtures unchanged. Rustfmt and new issue/spec/handoff relative links pass. The optional --expanded study was not rerun; its historical evidence is unchanged.

Implementation is 0a737f66; 9cb4c346 and ready HEAD add evidence, handoff/lifetime notes, status and issue closure. Code/tests/fixtures/check script/Cargo/toolchain are unchanged from the tested source. The built source was merged/pushed to fast-test before affected checks; final records were merged as fast-test **63109bf2**. The only final staging conflict was simultaneous insertion of finished-task rows; both APP 173-APP-0092 and NRL 900-NRL-0025 were retained. No build/test/gate ran on fast-test.

Evidence on the ready branch:

- `nrl/reports/native-conformance-2026-10-29.json` — tested source/toolchain, checks, exact hashes and delivered-message commits.
- `nrl/reports/native-conformance-check-2026-10-29.txt` — raw affected-check log.
- `nrl/reports/native-conformance-cases-2026-10-29.json` — 16 host runtime scenario outcomes/charges.
- `nrl/tests/fixtures/native-conformance-v0/bundle.json` and `nrl/specs/native-conformance-v0.md` — pinned model/independent payloads/encoding and native acceptance contract.
- `issues-done/900-NRL-0025-native-conformance-and-app-handoff.done` — resolution; 900-NRL-0017 remains open.

APP message: `APP/2026-10-29T1459Z-NRL-text-f64-agreed-pinned-native-conformance-package.md` (eaa3fc09). KRN notice: `KRN/2026-10-29T1459Z-NRL-nnd-format-agreement-and-client-identity-lifetime.md` (93568a17). The supplied model ID is `1b23aaf4baa75fa146ee8618ea1f7e0e64699c1ea2ee7cf27eeb8bc83f0c1bee`; reference comparison is finite/count checked, abs error <= 1e-12 + 1e-10*abs(reference).

No kernel/ABI/IDL/APP code or OS profile guarantee changes. **Native nnd/parser/target execution is still unimplemented/unverified** under 262-APP-0112 and 262-KRN-0178; 0179 is not a stage-0 blocker. The full repository gate remains yours; NRL affected checks do not replace it. CI/local gate already invokes the NRL script through 900-KRN-0174.
