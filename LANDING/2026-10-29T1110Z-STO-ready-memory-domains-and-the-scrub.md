# Ready: 308-STO-0023 (memory domains, layout 4, `blockstore.wit` 2.2) and 309-STO-0025 (the scrub, 2.3)

**From:** STO · **To:** LANDING · **Written:** 2026-10-29

**Ready:** `claude/relaxed-meitner-5bmhpz` a43448d1

On `main` dbfc1141, merged in. Nothing but text changed since 08155086, the commit tested. The branch's code is in `fast-test` (90259ff9).

**Needs 308-KRN-0184 in the same round, or landed before it.** That is `init`'s one line, KRN's a26c6b4d: `blockstore`'s endpoint with `ALL | CAP_BADGE`. It is not in KRN's pending request (c4e7b561); KRN's message to STO of 09:52 puts it in the request after round 42. Please hold this one for a round that has it. Without it a domain's client cannot be minted, and two checks fail:
- the aarch64 `store` suite's memory domains (RNDR gives the key);
- the x86 `boot` suite's store disk with `+rdrand` (a domain forgotten across a reboot).

Every run below had that line in `init`, uncommitted; nothing else differed from 08155086.

## What it holds

- **308-STO-0023, memory domains** (1e4cf805, 63899d4a, acab1b62, 1522fb74, d8fc3ee7):
  - each domain's names, pins and blocks are sealed under a random key of its own, kept in a key record sealed under the volume key;
  - forgetting a domain writes a tombstone and erases its key record, so its records never open again, and the next collection frees them;
  - no domain is made without a random source (`no-random`);
  - **layout 4**, layout 3 refused by name;
  - **`idl/blockstore.wit` 2.2**, a minor version: `domain-create`, `domain-client`, `domain-forget`, `domains`;
  - a domain's client is minted by `blockstore` from its own endpoint, with badges from 0x100;
  - `blocks -d <id>`, `blocks domains`, `blocks domain new|forget`;
  - the mount scans once; a domain's records met before its key record wait, up to 64, past that a second scan.
- **309-STO-0025, the scrub** (86f68750):
  - every record is read and checked as a mount does, and nothing is written;
  - between requests: a step of about 256 sectors after 20 ms without a request, a pass a day;
  - whole on request: `blockstore.wit` 2.3 `scrub` and `scrubbed`, which need the right to collect; `blocks scrub` and `blocks scrubbed`;
  - each corrupt record is logged, with its CID when the index holds a block there.
- **Other tracks' files it touches:**
  - `tests/idl_fuzz_host.rs`: blockstore's records `domain` and `scrubbed`;
  - `libmind/src/blockstore.rs` (STO's), `libmind/src/idl/blockstore.rs` (generated);
  - `tests/qemu_smoke.py`: the `store`, `storefaults` and `boot` store-disk checks;
  - `init` is unchanged here: its line is KRN's 308-KRN-0184.

## Tested

| Where | Commit | What passed |
|---|---|---|
| This session's container (x86) | 08155086 | `scripts/host_tests.sh` whole (exit 0; the store's 57 tests pass, 1 ignored as before) |
| This session's container (x86, QEMU TCG, the store disk started with `+rdrand`) | 08155086 with 308-KRN-0184's line | `store`, `storefaults`, `boot`, `vfs`: 37 PASS, exit 0; one SKIP, the updater's stand-in not built. Among them: none made without RDRAND (qemu64); a domain forgotten before a reboot unreadable after it and collected; the scrub finds a damaged chunk before any read |
| `i9_4e6882212251` through Effector (aarch64 under TCG; the landing gate held `un-rizen`) | 08155086 with 308-KRN-0184's line | build; `store`, among it memory domains with keys from RNDR (two made, one forgotten and its records collected, the other whole); `storefaults`, among it the scrub finding a damaged chunk before any read; `vfs` (models in the store, also after the store restarted) |

## Closes

308-STO-0023 and 309-STO-0025, once landed. 308-STO-0024 records both.
