# 351-ASR-0006 — The update threat model and fuzzing of the metadata parser

**Type:** assurance · **Owner:** `ASR` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** [351-UPD-0005](../issues-done/351-UPD-0005-release-and-publish.done) (the formats) · **Main task:** [351](351-self-update.md) · **Constitution:** MC-9.4, MC-9.6, MC-11.10, MC-12.2

Numbered by the kernel session at the maintainer's request (2026-10-08); the track is open.

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

## Related

[351](351-self-update.md), [500](500-fuzzing-abi-and-idl.md).
