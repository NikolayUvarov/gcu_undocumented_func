# 801-UPD-0039 — The updater always reads the cable's directory, `data/link/rel/`, whatever `update.txt` says

**Type:** update (`updater`) · **Owner:** `UPD` · **Priority:** P1 · **Status:** open (2026-11-14) · **Blocked by:** — · **Main task:** [801](801-the-bench-stays-manageable.md) (S6) · **Roadmap:** stage V · **Constitution:** MC-9.2, MC-9.3, MC-9.4, MC-12.3

## Problem

LNK's request (`UPD/2026-11-14T0944Z-LNK-801-the-updater-always-reads-data-link-rel.md`), for [801-LNK-0044](801-LNK-0044-the-cable-updates-everything-with-no-permission-words.md), under the maintainer's decision of 2026-11-14: the cable updates all code with no permissions, and releases stay signed.

The updater's only source was the one `update.txt` names. A release put into `data/link/rel/` through the cable was taken only where `update.txt` said `source data/link/rel` with `automatic apply`. It was ignored:
- with no `update.txt`, where the updater logged `NO update.txt: IT ACTS ONLY WHEN ASKED, AND HAS NO SOURCE`;
- with an `https://` source;
- with `automatic none` or `check`.

## Plan

- **`data/link/rel/` is a source always.**
  - **When:** every 20 s while no trial waits, the updater opens `timestamps/<channel>` there. Once that file changed since the last look, it runs `check`, `fetch` and `apply` with the directory lent as the source, as an automatic `apply` does, so the log lines are the same.
  - **A directory with no timestamp** costs one open and logs nothing.
  - **A release refused** for its signature, its expiry or a version not newer is said once, until the timestamp changes.
  - **A release still being put** (a file missing or short) is looked at again at the next turn.
- **Every check is any source's** (MC-9.2–9.4). So the cable installs only what the updater's keys signed:
  - the key file, taken as the next of the one trusted;
  - the timestamp's and the channel's signatures and expiry;
  - the manifest's boot-key signature;
  - each file's size and SHA-256;
  - the floor and the minimum.
- **The channel** is `update.txt`'s (`stable` without the file).
  - The timestamp's and the recovery orders' records are kept by channel, so the cable and a network source of the same channel share them.
  - A newer release from either wins.
- **An `update.txt` naming the directory** as its `source` leaves it to this path, whatever its `automatic` says.
- **The log:**
  - `[UPDATER] SOURCE data/link/rel (THE CABLE'S, 801-LNK-0044): ALWAYS, AUTOMATIC APPLY` at the start in a slot;
  - with no `update.txt`, `[UPDATER] NO update.txt: IT ACTS BY ITSELF ONLY ON THE CABLE'S data/link/rel`.
- **No IDL change:** `check` and `fetch` asked without `update.txt` still answer `no-source`, as `update.wit` says.

## Acceptance criteria

1. **The `linkupdate` suite, x86 and aarch64:**
   - LNK's `cable_without_words_check` prints its `PASS: cable without words: …` line, with 801-KRN-0247's half in too: on a disk with no `update.txt`, release 6 is put into `data/link/rel/` over the link, taken into slot B, booted on trial and confirmed;
   - the existing `linkupdate_suite`, whose `update.txt` names `data/link/rel`, still passes, its refusals (expired, changed signature) included.
2. **The `updater` suite** still passes.
3. **The documents:** `docs/update/updater.md` (EN and RU) and the threat model's cable entry say what the updater does.

## Related

[801](801-the-bench-stays-manageable.md), [801-LNK-0044](801-LNK-0044-the-cable-updates-everything-with-no-permission-words.md), [801-KRN-0247](801-KRN-0247-init-starts-the-cable-with-no-word-on-the-disk.md) (`init`'s half), 351-KRN-0141.
