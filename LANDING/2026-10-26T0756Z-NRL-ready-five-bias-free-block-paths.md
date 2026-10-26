# Ready: five bias-free NRL block paths

**From:** NRL · **To:** LANDING · **Written:** 2026-10-26 07:56 UTC · **Concerns:** 900-NRL-0001 through 0007; parent 900 remains open

**Ready:** `codex/NRL-lab` e6135049be76d0cbd51007e0fbbc0523deb5db98

This replaces the unanswered request `LANDING/2026-10-25T2058Z-NRL-ready-first-nrl-host-block.md` for the same branch. The maintainer explicitly deferred landing for a couple of days and asked NRL to continue autonomously; no immediate round or response is requested, and further host work does not wait for landing.

The branch now contains all five bias-free block execution paths: dense/pruned SiLU, linear and cubic Hermite splines, and directly executed SVD factors. It includes stable shared channel ranking, analytic gradients, bounded block artifacts and a small host-only Jacobi SVD initializer (dimensions <=64, at most 128 sweeps; numerical failures/nonconvergence are errors). Current block schema 3 replaces earlier in-project prototype schemas with explicit refusals and regeneration, without compatibility readers.

Validation on NRL's own branch at d7dbbefa, after fast-test staging 4e349557: **25 Rust host tests**, portable release rlibs for x86_64-unknown-none and aarch64-unknown-none-softfloat, and five external forward/backward/SGD/exact-round-trip cases passed. Every factor has an independent finite-difference check. Nine small matrices compare singular values, reconstruction and truncated residuals to a separate torch.linalg.svd f64 oracle. Fixture regeneration was byte-identical. No build/test ran on fast-test.

Evidence: `nrl/reports/svd-f64-2026-10-26.json`, `svd-check-2026-10-26.txt`; previous scoped runs are recorded alongside them. The later commits only close issues, record evidence/status and extend THIRD_PARTY attribution; numerical source and fixture bytes remain the tested ones.

Please run `bash nrl/tools/check.sh` explicitly as part of the eventual candidate checks until KRN wires it into CI. The existing request is on main fb821bbd in `issues/requests-KRN.md`. THIRD_PARTY.md is image-copied, so this package requires the full landing gate and has no text-only exemption.

Limits: no general affine/bias conversion, QSL/full-model importer, real trained-model quality/size/speed result, native linked/run NRL app, nnd service or GPU execution. All five paths remain a small scalar CPU f64 research profile. The portable builds are rlibs only. Parent 900 and its general T01/T03/model/native acceptance criteria stay open.
