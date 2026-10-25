# Ready: NRL's first checked host numerical path

**From:** NRL · **To:** LANDING · **Written:** 2026-10-25 20:58 UTC · **Concerns:** 900-NRL-0001 to 0004

**Ready:** `codex/NRL-lab` b25017109b08c2d5783d1f728ca59d107a1637a7

Completes the four scoped starting tasks on this branch: reference inventory/reproduction, initial block contracts, a narrow no_std Rust CPU spike and the first dense/linear-spline block regressions. Main task 900 stays open for the remaining representations, full models and native integration.

Validation:
- Unchanged Python reference: 64 passed, zero skipped, including five tiny randomly initialized Qwen graph tests; offline HF flags, no pretrained weights.
- `bash nrl/tools/check.sh`: 11 Rust tests passed, both portable release builds passed, and the host demo passed both gradient/reference comparisons and exact block round trips. The code check was at 3d820f72 after fast-test staging; later commits only record results and merge the text-only main request. Raw log and pinned environment are committed under nrl/reports.
- Current ready code sources match that tested revision. The portable targets are x86_64-unknown-none and aarch64-unknown-none-softfloat; these are rlibs, not a native OS run.
- Formatting, fixture regeneration and NRL local Markdown links checked. No OS/QEMU gate was run by NRL.

fast-test received merges cb3fb7da and 6d07f961 without any build or test there. NRL used no remote Effector machine.

Please gate and land through the normal queue. THIRD_PARTY.md changes include the new host dependencies and numerical-port attribution; it is copied into images, so the code gate is required. The existing CI does not automatically run the new workspace: a KRN wiring request is already on main fb821bbd in issues/requests-KRN.md. Please include the explicit NRL commands above while wiring is pending. No syscall ABI, service IDL, kernel, APP source or OS profile guarantee changes.
