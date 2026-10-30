# 311-APP-0116 — The shell builds against `vfs.wit` 2.6's `conflict`

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P1 (311-KRN-0189 lands with it) · **Status:** in progress (the arm written; its words once 2.6 is in `main`) · **Blocked by:** — · **Main task:** [311](311-storefs-a-file-face.md) · **Roadmap:** track B · **Constitution:** MC-12.4

Numbered on 2026-10-30 from the kernel track's request in `requests-APP.md`, "An arm for `vfs.wit` 2.6's error `conflict` in the shell", for 311-KRN-0189 (STO's `storefs`, 311-STO-0032).

## Problem

- **What 2.6 changes.** `vfs.wit` 2.6 appends the error `conflict`, a write or `flush` refused because another writer published the file first, and `mind::fs::Error` gets `Conflict`.
- **Why the shell is in the way.** `shell/src/files.rs` `text` matched `mind::fs::Error` exhaustively, so the shell would not build with 2.6.

## Plan

- **Now:** a last arm, `#[allow(unreachable_patterns)] _ => "REFUSED"`, which builds against `main` as it is and against 2.6.
- **Once 2.6 is in `main`:** `Error::Conflict => "CHANGED BY ANOTHER WRITER; READ IT AGAIN"`, in place of the arm.

## Acceptance criteria

- The shell builds against `main` now. It builds against 311-KRN-0189's branch too.
- With 2.6: a write `storefs` refuses with `conflict` is named so in the shell (STO's check of `storefs`).

## Related

311-KRN-0189, 311-STO-0032.
