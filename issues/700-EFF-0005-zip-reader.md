# 700-EFF-0005 — `mind::zip`: a bounded ZIP reader

**Type:** library (`libmind`) · **Owner:** `EFF` track (open) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [700](700-effector-agent.md) · **Constitution:** MC-11.5, MC-11.11, MC-5.1

## Problem

- **Format.** Effector delivers a deployment package as a ZIP archive with `manifest.json` at its root. MIND Core has no ZIP reader.
- **Hostile input.** An archive from the network can be a zip bomb, hold paths that escape the target directory, or lie about its sizes.

## Plan

- **`libmind/src/zip.rs`, `no_std`.**
- **Reading:**
  - the central directory;
  - the stored and deflate methods, with deflate from `miniz_oxide` (`no_std`; its licence goes in `THIRD_PARTY.md`);
  - CRC-32 checked;
  - ZIP64 refused unless needed.
- **Limits:**
  - number of entries;
  - name length;
  - uncompressed size per entry and in total;
  - compression ratio.
- **Names:**
  - relative, `/`-separated, no `..`, no absolute paths, no drive letters, no duplicates;
  - a name the target volume cannot hold is refused.
- **Encrypted entries are refused.** The agent accepts only packages whose authenticity rests on a MIND Core signature (0008), not on an archive password.
- **Host tests:**
  - archives made by `zip` and by 7-Zip;
  - stored and deflate entries;
  - bombs, traversal names, bad CRCs and truncated files refused.

## Acceptance criteria

- The host tests pass in `scripts/ci_local.sh`.
- `THIRD_PARTY.md` records `miniz_oxide`.
- A fuzz target is listed for 500.

## Related

[700](700-effector-agent.md), [700-EFF-0008](700-EFF-0008-application-packages.md), [500](500-fuzzing-abi-and-idl.md).
