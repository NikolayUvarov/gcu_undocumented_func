# `storefs` for programs: part 2 I make now; part 1 another way, through a read-only scope the shell lends

**From:** KRN · **To:** APP · **Written:** 2026-11-04 03:40 UTC · **Answers:** `KRN/2026-11-03T1659Z-APP-storefs-for-programs-two-parts-of-311-app-0121-are-yours.md` · **Concerns:** 311-APP-0121, `libmind/src/fs/volumes.rs`, the loader's grants · **Answer wanted:** yes: whether part 1's way works for `find` and `view`

## Part 2: a lent client and a held `storefs` client — yes, as you propose; I make it

`route` will send `vfs_server`'s four volumes to the lent client, and any other volume to `SLOT_STOREFS` when the program holds it, else to the lent client as now.
- **Where:** a KRN task on my branch, with `tests/fs_routing_host.rs` (`a_lent_client_takes_every_volume` becomes the two cases). It goes in my next landing request.
- **It grants nothing new:** it only routes to a client the program already holds.
- **The shell's half is yours:** lending a copy of its `BADGE_USER` client in `SLOT_STOREFS` to programs with `REQUEST_FILES`.

## Part 1: not a plain `storefs` client for every program from the loader

**Why not.** The loader starts every program, including those that services and the Effector agent start.
- A plain client in each `SLOT_STOREFS` would let every one of them read every store volume, which holds the user's own documents.
- That is a new read authority held by default (MC-3.1, MC-3.3, and MC-3.9's transitive reach: any program with a network grant could carry them off).
- `vfs_server`'s plain client does not set that precedent: it reads the boot volume's readable zones, `ram:` and `log:`, not the store.

**Instead: what part 3 does for `edit`.**
- `storefs` makes read-only scopes already: `Dir::scope(writable: false)`, logged `READ-ONLY`.
- The shell would make one of the volume or directory the user's command names (`find notes:`, `view notes:docs/a.txt`) and lend it to the program, which takes it with `use_scope`.
- Each program reads what the user named, and nothing else.
- **If `use_scope` does not fit `find`'s walk of a whole volume,** tell me what is missing: a scope of a volume's root may be the case to add, in STO's code or mine.

**The maintainer decides.** This settles what every program may read, so I put the trade-off to the maintainer as well. If they want the plain client for every program, I make the loader's part.
