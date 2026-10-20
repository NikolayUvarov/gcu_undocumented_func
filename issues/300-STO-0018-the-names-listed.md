# 300-STO-0018 — The current names listed by prefix, page by page (`blockstore.wit` 1.4)

**Type:** storage (service, interface, tool, tests) · **Owner:** `STO` track · **Priority:** P2 · **Status:** in progress (built; host tests pass; the store suite's check written) · **Blocked by:** — · **Main task:** [300](../issues-done/300-checksummed-block-store.done) · **Roadmap:** track B · **Constitution:** MC-4.3, MC-4.7

From the request "A list of the published names (for `fm`'s `store:` panel)" in requests-STO.md (recorded by the tools track, 2026-10-09, for [300-APP-0038](../issues-done/300-APP-0038-block-store-panel-in-fm.done)), which this task takes from that file.

## Problem

- `fm` shows the block store as the volume `store:`. Published names are its files, and a `/` in a name makes directories.
- `idl/blockstore.wit` 1.3 can resolve a name, but no call lists them; `stat` gives only their number.
- So `fm` lists only the names it has itself published or opened in that run, and the objects the caller's owner pinned. A name published by `blocks`, `tally` or the updater stays out of sight until someone types it.

## Plan

- **The interface.** `names: func(prefix: string<64>, after: string<64>) -> result<list<named, 16>, error>` in `blockstore.wit` 1.4, with `record named { name, version, root }`:
  - up to 16 current names that start with `prefix`, in byte order, each after `after` (empty: from the first), each with its version and root;
  - a removed name is not listed;
  - the next page is asked for after the last name of a page, until one has fewer than 16;
  - `invalid` for a `prefix` or `after` with a byte no name has, or longer than 64.
- **Rights.** `BADGE_GET`, as `resolve` (`mind::blockstore::Operation::Names`). Knowing a name grants nothing: reading its object still needs the right (MC-4.7).
- **The service.** The store keeps its names sorted (`Store::names`), so those with a prefix are one run found by a binary search.
- **The tool.** `blocks names [prefix]` asks for every page and prints each name (`<name> VERSION <n> ROOT <cid>`), then `<n> NAMES IN <p> PAGES`.
- **`fm`'s panel** is the tools track's: it can ask for the names now (`fm/src/store.rs`, `names`).

## Acceptance criteria

- [x] Host tests: names by prefix in byte order whatever the order of publishing, pages of 16, a removed name left out, a start that is not a name, `invalid` for a bad prefix or start, the same list after a mount (`names_are_listed_by_prefix_in_pages`); the right is `BADGE_GET`'s (`rights_come_from_the_badge`).
- [ ] The store suite lists 18 names by prefix in 2 pages and in order, every current name without the removed ones, and a prefix through the client that may only read (x86 and aarch64).

The request's other criterion, `fm`'s `store:` panel showing a name that `blocks` published, is the tools track's part (300-APP-0038's remainder); the tools track is told when `blockstore.wit` 1.4 is in `main`.

## Related

- [300-APP-0038](../issues-done/300-APP-0038-block-store-panel-in-fm.done): `fm`'s `store:` panel.
- [docs/storage](../docs/storage/README.md), "Names".
