# 300-APP-0101 — `fm`'s `store:` panel lists every published name

**Type:** tools (`fm`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [300](../issues-done/300-checksummed-block-store.done) · **Roadmap:** track B · **Constitution:** MC-4.2, MC-4.7

**Recorded by:** the tools track, 2026-10-21, after the storage track's answer (STO → APP, 2026-10-21 00:34 UTC): `blockstore.wit` 1.4 lists the published names ([300-STO-0018](../issues-done/300-STO-0018-the-names-listed.done)). The request's second criterion is this track's: the panel shows a name that `blocks` published.

## Problem

- **The panel shows only what fm saw.** `blockstore.wit` 1.3 had no list of names. So `store:` showed the names fm itself published or opened in that run ([300-APP-0038](../issues-done/300-APP-0038-block-store-panel-in-fm.done)).
- **What the panel misses:** a name published by `blocks`, `tally` or another program is not there until fm opens it by path.

## Plan

- **The list from the store.** `fm`'s store client lists with `names(prefix, after)`, 16 names a page, paging after the last name until a page has fewer than 16, at most 256 pages.
- **An older store** answers no list, and `fm` falls back to the names it saw.
- **Docs:** `docs/tools/README.md` and `README_RU.md`.

## Acceptance criteria

- **Host tests** (`tests/fm_host.rs`): a store that lists its names and one that cannot. These tests exist since 300-APP-0038.
- **The store suite in QEMU:** `fm_store_check` finds `copy` and the directory `list` on `store:` after `blocks` published them. In `list`, it finds `n17`, which is on the store's second page.

## Progress (2026-10-21)

- **Done:** `fm`'s store client pages through `names`.
- **The host tests pass.**
- **Locally,** "x86: files and block writes" passed on e681330f: `fm_store_check` found `copy` and, in `list`, `n17`.

## Related

[300-APP-0038](../issues-done/300-APP-0038-block-store-panel-in-fm.done), [300-STO-0018](../issues-done/300-STO-0018-the-names-listed.done).
