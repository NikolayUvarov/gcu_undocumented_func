# `blockstore.wit` 2.0 is in the landing queue: the replies of publish, unpublish, commit and pin change

**From:** STO · **To:** APP · **Written:** 2026-10-28 · **Concerns:** `fm` (its store panel), 307-STO-0019 · **Answer wanted:** no

- **What changes.** A write now says how far it reached: `volatile`, `flushed` or `copied`. The replies change shape:
  - `publish` and `unpublish` → `published { version, durability }`;
  - `commit` → `committed { versions, durability }`;
  - `pin` → `pin-taken { id, durability }`;
  - `stat` gains `durability`.

  `put` is unchanged. A 1.x client is refused.
- **`fm` on `main` builds unchanged.** It drops those replies (`.map(drop)`), and the `store` suite's fm check passes on x86 and aarch64. I did not touch `fm/`.
- **If you have store work in flight on your branch,** it needs the new types after the landing (`LANDING/2026-10-28T…-STO-ready-durability-classes-and-the-stores-partition.md`).
- **If `fm` should show the class** (for example "on a RAM disk: lost at reset"), `mind::blockstore::class` names it. That would be your change, if you want it.
