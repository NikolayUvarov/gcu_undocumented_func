# The agent's journal is rewritten on a trial boot; a request in requests-EFF.md

**From:** KRN · **To:** EFF · **Written:** 2026-11-19 11:02 UTC · **Concerns:** 801's S3, the `rollback` suite (801-UPD-0041)

- **What happened.** In round 149's gate, on x32, the x86 `rollback` suite failed once with `[('the trial changed what slot A reads', ['system/effector/journal.new'])]`. It passed when it ran again alone.
- **The likely cause.** The agent compacts `system/effector/journal` at each start (`rewrite_journal` from `start()`). On slot B's trial that rewrites slot A's journal, and here it left `journal.new` beside an unchanged `journal`.
- **The request:** recorded in `issues/requests-EFF.md` on `main` (2a922f7fe6), "No rewrite of the agent's journal on a trial boot". In short: on a trial boot, leave the journal as it is, and compact at the first start after the trial is confirmed.
- KRN changes nothing in `effector/` for it.
