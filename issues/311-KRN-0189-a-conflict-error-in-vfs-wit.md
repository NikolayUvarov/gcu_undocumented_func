# 311-KRN-0189 — `vfs.wit` 2.6: an error `conflict`, for a write another writer published before

**Type:** service interface (`idl/vfs.wit`, `libmind` `fs`) · **Owner:** `KRN` · **Priority:** P2 · **Status:** open · **Blocked by:** an arm for the new error in the shell (APP, [requests-APP.md](requests-APP.md)) and in `effector_gw` (EFF, [requests-EFF.md](requests-EFF.md)), each in `main` or in the same landing round · **Main task:** [311](311-storefs-a-file-face.md) (STO's), for 311-STO-0032 · **Constitution:** MC-11.10, MC-12.4

**Asked by:** the storage track (`STO`) in `issues/requests-KRN.md`, 2026-10-30, "`storefs`: started by `init`, reached by a files client, and a `conflict` error in `vfs.wit`", and in its message to KRN of 10:53.

## Problem

`storefs` (311-STO-0032) publishes each `flush` and `close` by compare-and-swap (311-STO-0029). A writer that read an older version is refused. `vfs.wit` 2.5 has no error for that: `io` would say the medium failed, and `exists` that a name is taken. The writer could not tell that it should read the file again (MC-11.10: a change of the source has a defined outcome).

## Plan

- **`idl/vfs.wit` 2.6.0:** `conflict` appended to `enum error`, so the other codes keep their numbers. Its comment says that `vfs_server` never answers it.
- **Generated code:** `libmind/src/idl/vfs.rs`, regenerated with `scripts/mind_idl.py`.
- **`mind::fs::Error::Conflict`**, mapped from it. A system error code for it, where one is needed: `Invalid`, as for the other refusals.
- **Users of the error that match it exhaustively** need an arm:
  - `shell/src/files.rs` `text` (APP);
  - `effector_gw/src/main.rs` `vfs_fail` (EFF).

  Both are requested with the lines. `link` and the kernel track's own users have a catch-all, or get the arm here.
- **`docs/idl/README.md`:** the row reads 2.6.0. It also names 2.5's `adopt` (000-KRN-0170), which the row lacks.

## Acceptance criteria

- `tests/idl_fuzz_host.rs` decodes 2.6's error in every reply. A code past `conflict` is refused.
- A host test: `mind::fs` maps `conflict` to `Error::Conflict`.
- Every program builds, x86 and aarch64. The groups `files and block writes` (x86) and `files and the block store` (aarch64) pass unchanged: `vfs_server` answers as before.
- STO's `storefs` answers `conflict` on the wire. That is 311-STO-0032's criterion; this task only makes it expressible.

## Related

[311](311-storefs-a-file-face.md), 311-STO-0032, [311-STO-0029](311-STO-0029-a-working-tree-published-by-compare-and-swap.md), [311-KRN-0190](311-KRN-0190-storefs-at-boot-and-its-clients.md).
