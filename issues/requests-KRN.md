# Requests for the kernel track (KRN), not numbered yet

**Owner:** kernel track · **Status:** open (1 request waiting, 2026-10-10, until `tests/idl_fuzz_host.rs` reaches main; the toolchain installed once became [000-KRN-0060](000-KRN-0060-toolchain-once-before-the-parallel-build.md); the TPM's registers became [351-KRN-0052](351-KRN-0052-tpm-registers-from-the-firmware.md), `bcm_wifi` as a boot service [550-KRN-0059](550-KRN-0059-bcm-wifi-at-boot.md), and the tools track's `SLOT_SHELL` and `SLOT_CLIPBOARD` from its branch [211-KRN-0058](211-KRN-0058-slots-for-the-shell-and-the-clipboard.md)) · **Recorded by:** the tools track (APP), 2026-10-06

The kernel track numbers its own tasks (`NNN-KRN-MMMM`), so requests from other tracks wait here. The kernel track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## The IDL fuzzer in CI's host tests (500-ASR-0001)

**Recorded by:** the assurance track (`ASR`), 2026-10-09, for [500-ASR-0001](500-ASR-0001-idl-decoder-fuzzing.md).

### Problem

`tests/idl_fuzz_host.rs` fuzzes every generated IDL decoder with a fixed seed: 24 receivers and 82 types, 50 000 inputs per target, about 4 s. It is not in CI's host tests yet, and the CI files are the kernel track's.

### Plan (a proposal; the kernel track decides)

- Add `idl_fuzz` to the list of host tests in `.github/workflows/ci.yml` (the step "Host tests") and in `scripts/ci_local.sh` (`host_tests`), built like the others: `rustc --edition=2021 --test tests/idl_fuzz_host.rs`.
- If 175-KRN-0046 changes how the host tests fail, the line follows that.

### Acceptance criteria

CI runs the test on every push, and a finding fails the host-test step.
