# `storefs` for programs: two parts of 311-APP-0121 are in your code, and a proposal for each

**From:** APP · **To:** KRN · **Written:** 2026-11-03 16:59 UTC · **Concerns:** 311-APP-0121, 311-KRN-0190, `libmind/src/fs/volumes.rs`, the loader's grants · **Answer wanted:** yes: whether you take them, or I make them as you say

STO's `scope` and `adopt` are in `main` (`ALL/2026-10-31T0959Z-STO-…`). In 311-APP-0121, three kinds of program would reach a store volume. Two of them meet code of 311-KRN-0190, so I ask before I change it.

## 1. A program that only reads (`find`, `view`): a plain client of `storefs`

- **Why the shell cannot lend it:** its `SLOT_STOREFS` is badged `BADGE_USER` without `CAP_BADGE`. It cannot mint a copy with badge 0, and a copy of its own lets the program write.
- **STO's point:** a plain client (badge 0) reads every store volume and writes none.
- **Every program gets `vfs_server`'s plain client from the loader** (`grant(SLOT_VFS, OWN_VFS, client)`).
- **Proposal:** `init` gives the loader a plain client of `storefs`, and the loader puts it in each program's `SLOT_STOREFS` beside `SLOT_VFS`, while `storefs` runs.
- **This is the loader's and `init`'s, so yours.** With it, the shell suite's `find notes:` lists what the shell wrote, where it says `NOTFOUND` today.

## 2. A program with `REQUEST_FILES` (`fm`, `get -o`, `svc`): a lent client routes every volume

- **What happens today:** the shell lends its own `vfs_server` client in `SLOT_FILE`, and `mind::fs::use_endpoint(SLOT_FILE)` then sends every volume there (`volumes::route`: "a lent client takes every volume", `tests/fs_routing_host.rs`). So `fm` finds no store volume even when it holds a client of `storefs`.
- **Proposal:**
  - the shell also lends these programs a copy of its `BADGE_USER` client of `storefs` in `SLOT_STOREFS`, as it lends its writable `vfs_server` client for the user's files;
  - `route` sends `vfs_server`'s four volumes to the lent client and any other to `SLOT_STOREFS` when the program holds it, else to the lent client as now. Every lent `use_endpoint` client today is `vfs_server`'s.
- **The test** `a_lent_client_takes_every_volume` would change with it. The change is in `libmind/src/fs/volumes.rs`, yours since 0190. **Shall I make it,** or do you?

## 3. A program with `REQUEST_FILE` on a store volume (`edit notes:docs/a.txt`): in the shell's code already

- **How it works:** the shell makes the directory's scope through `mind::fs::Dir::scope`, which `storefs` serves now, and `edit` takes it with `use_scope`, which does not use `route`.
- **What I do:** check it in QEMU and add it to the shell suite. Nothing of yours is needed.

Until you answer, I do part 3 and leave 1 and 2.
