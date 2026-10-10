# Requests for the state and recovery track (STO), not numbered yet

**Owner:** state and recovery track · **Status:** open (1 request waiting, 2026-10-10)

The STO track numbers its own tasks (`NNN-STO-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## A list of the published names (for `fm`'s `store:` panel)

**Recorded by:** the tools track (APP), 2026-10-09, for [300-APP-0038](../issues-done/300-APP-0038-block-store-panel-in-fm.done).

### Problem

`fm` now shows the block store as the volume `store:`. Published names are its files, and a `/` in a name makes directories. `idl/blockstore.wit` 1.3 can resolve a name, but no call lists them: `stat` gives only their number. So `fm` lists only the names it has itself published or opened in that run, and the objects the caller's owner pinned. A name published by `blocks`, `tally` or the updater stays out of sight until someone types it.

### Plan (a proposal; the storage track decides)

- `names: func(prefix: string<64>, after: string<64>) -> result<list<head-named, 16>, error>` in `blockstore.wit` 1.4: the current names that start with `prefix`, in byte order, after `after`, page by page. Each comes with its version and root.
- It needs `BADGE_GET`, as `resolve` does. Knowing a name grants nothing: reading the object still needs the right (MC-4.7).
- `fm::store::Store::names` then asks for them, and `fm` shows every name.

### Acceptance criteria

- The store suite lists the names after publishing several, by prefix and in pages of 16.
- `fm`'s `store:` panel shows a name that `blocks` published.
