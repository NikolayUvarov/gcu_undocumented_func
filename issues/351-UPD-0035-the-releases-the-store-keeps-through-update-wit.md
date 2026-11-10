# 351-UPD-0035 — The releases the store keeps, through `update.wit`

**Type:** `updater`, interface · **Owner:** `STO`, by agreement with the update track's owner (the kernel session, 2026-11-10) · **Priority:** P3 · **Status:** in progress (`kept` in `main` since round 116, 2026-11-10; the `updatestore` suite's read through the shell waits for APP's command) · **Blocked by:** — · **Main task:** [351](351-self-update.md) · **Constitution:** MC-12.3, MC-12.4

Numbered from STO's offer of 2026-11-09 (`KRN/2026-11-09T1925Z-STO-two-updater-follow-ups-of-351-upd-0033-an-offer.md`), the second of 351-UPD-0033's "not done" points. STO makes it.

## Problem

Which releases the store keeps for the updater (the running one and the last known good one, 351-UPD-0033) shows only in the updater's log. No client can ask for them.

## Plan

1. **A call `kept`** (`update.wit` 1.5, minor), for any client of the updater: each kept release's version, pin, root and bytes.
2. **For APP:** a request that the shell's `update` shows them.

## Acceptance criteria

- **A host test** covers the call's encoding.
- **The `updatestore` suite** reads the kept releases through the shell, once APP has the command.

## Progress (2026-11-10)

**Made by STO** on `claude/relaxed-meitner-5bmhpz`, with 351-UPD-0034 in `update.wit` 1.5.
- **`kept: func() -> list<kept-release, 8>`**, any client's:
  - the releases the store keeps, newest first, at most 8, and none without the store;
  - `kept-release` is the version, the pin, the tree's root (a CID in binary, 36 bytes) and the bytes of its blobs, each counted once.
- **`updater/src/main.rs`** answers it from the store's pins (`releases_kept`). A tree whose blobs are not read gives 0 bytes, and the log says so.
- **`tests/updater_stub`**, which serves `update.wit` in the suites that need no real updater, answers an empty list.
- **Docs:** `docs/update/updater.md` and `_RU` 0.10, `docs/idl`.

**The first criterion is met:**
- `tests/idl_host.rs` round-trips none, two and eight releases through the generated client and server, a root's 36 bytes and a size past 32 bits among them;
- a ninth is refused, the most the list holds being eight;
- `tests/idl_fuzz_host.rs` decodes the record among every generated type.

**Left: the second criterion,** the `updatestore` suite reading the kept releases through the shell. That waits for APP's command, asked once this lands (KRN's word, 2026-11-10).

**Landed** in round 116 (`main` 46d756be, 2026-11-10; STO's request of 1642Z): every group of the gate passed, "x86: RAM above 4 GiB" on its second run alone, its first having stopped at a program's start, 000-KRN-0232. **Left:** the second criterion, as above.

## Related

[351-UPD-0033](../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done), [351-UPD-0034](../issues-done/351-UPD-0034-apply-writes-a-damaged-staged-slot-again-from-the-store.done).
