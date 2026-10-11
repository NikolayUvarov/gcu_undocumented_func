# 351-ASR-0006 — The update threat model and fuzzing of the metadata parser

**Type:** assurance · **Owner:** `ASR` · **Priority:** P2 · **Status:** in progress · **Blocked by:** [351-UPD-0005](../issues-done/351-UPD-0005-release-and-publish.done) (the formats) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.6, MC-11.10, MC-12.2

Numbered by the kernel session at the maintainer's request (2026-10-08). Taken by the assurance track (2026-10-09).

## Problem

The updater parses data that comes from the network before it verifies it: the channel file, manifests, and SSH and HTTP framing. The known attacks on update systems have to be named, each with the check that stops it:

- rollback;
- freeze, where old metadata is served forever;
- mix-and-match of files from different releases;
- endless data;
- slow retrieval;
- key compromise.

## Plan

- **`docs/assurance/update-threats.md`:** each attack, the check that stops it (which task, which test), and what is not covered. Physical rollback without a hardware counter is the first entry there.
- **Fuzzing** of the channel-file and manifest parsers and of the boot-record reader, with the harness of [500](500-fuzzing-abi-and-idl.md).
- **Limits:**
  - sizes stated for every field, and a download that exceeds the manifest's size is cut off;
  - a timeout for a slow server.

## Acceptance criteria

The threat table exists and names a test for every attack it says is covered. The parsers run under the fuzzer in CI for a fixed time without a finding.

## Progress

**2026-10-09:**

- **`docs/assurance/update-threats.md`:** each attack, what stops it, its test, and what is not covered. Physical rollback, the boot records and everything a device would check on a channel are not covered.
- **Fuzzers,** with fixed seeds and counts:
  - `tests/manifest_fuzz/` (the bootloader's manifest reader with its own crates, `cargo test --release`): no finding, and a reader that skips the hash was caught;
  - `tests/update_fuzz_host.rs` (the boot records and the slot choice);
  - `tests/update_fuzz_test.py` (the host's channel checker).
- **Two findings, sent to the update track** in requests-UPD.md, which numbered them [351-UPD-0014](../issues-done/351-UPD-0014-a-channel-checked-field-by-field.done) and [351-UPD-0015](../issues-done/351-UPD-0015-a-sequence-that-cannot-count-down.done), both done (2026-10-10):
  - `release.check` raises on signed channels it should refuse;
  - the trial's count-down stops at the largest sequence number.
- **Where it is.** The document and the fuzzers are on `claude/351-ASR-0006-update-threats`.
- **What remains:**
  - their CI lines (the kernel track's files): the manifest fuzzer at once, the other two once the update track's fixes pass them;
  - the device's channel parser, fuzzed when the updater (351-UPD-0007) has one.

## Related

[351](351-self-update.md), [500](500-fuzzing-abi-and-idl.md).
