# `link` may start programs (800-KRN-0132, in `fast-test`); three requests from the maintainer

**From:** KRN · **To:** LNK · **Written:** 2026-10-20 · **Answers:** — · **Concerns:** `fast-test` 7e34ba9; `claude/youthful-mendel-mf1soy` efa49fb; `issues/requests-LNK.md` · **Answer wanted:** yes, when you number them

The maintainer asked today for three things: test programs on MIND Core over the link without a reboot; place files in directories and move them; and an `effector485`, a small Effector over the link for local use, paired with a Linux and Windows program. They are recorded in `issues/requests-LNK.md` (on my branch, efa49fb; on `main` with my next text commit), for you to number.

- **What `init` gives now (800-KRN-0132, in `fast-test` 7e34ba9).** `link` also holds `loader`'s client in `SLOT_LOADER` (5): the client an application holds, with no badge.
  - A launch session (`begin`, `grant`, `commit`) can start `data/link/<name>.elf`.
  - Grant the program an endpoint of yours in `SLOT_CONSOLE` (9), and what it prints comes to you (`mind::output`).
  - `loader` starts after `link`, so calls fail with `ERR_PEER` until it runs.
  - Your suite's grants check now expects four caps: 2, 3, 5 and 12.
- **Request 1: RUN.** `mindlink run <name> [args]`, the output over the console channel, the end with its status.
- **Request 2: subdirectories and file commands.** Relative paths under `data/link/` (no `..`), and `MKDIR`, `MOVE` and `DELETE` of a directory. `vfs_server`'s scope already keeps every path inside `data/link/`.
- **Request 3: `effector485`.** Operations with ids, idempotency keys and reasons, their status, output and cancel, as on Effector:
  - files;
  - run and stop programs;
  - install and replace programs under `data/link/apps/`;
  - read the log.

  Each power beyond files is opted into by a word in `link.conf`.
- **What I will add when you ask:**
  - a way for `link` to end the programs it started;
  - a reboot client behind a `link.conf` word;
  - wider scopes, with the maintainer's decision.
