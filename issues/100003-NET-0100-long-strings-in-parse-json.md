# 100003-NET-0100 — `parse.json` reads strings longer than 512 bytes, in pieces that never split a character

**Type:** `parse`, IDL (`parse.wit` 5.1) · **Owner:** `NET` (the kernel session) · **Priority:** P1 · **Status:** in progress (made, 2026-11-28) · **Blocked by:** — · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), for [100003-APP-0151](100003-APP-0151-improve-on-mind-core.md) · **Constitution:** MC-11.11, MC-11.5, MC-12.4

## Problem

**APP's request of 2026-11-21**, recorded in `requests-NET.md` and taken that day (`APP/2026-11-21T1300Z-NET-re-parse-json-long-strings-taken.md`).
- `improve` on MIND Core reads the hub's JSON through `parse` (MC-11.11). It holds a long-term key and a network grant, so it must not parse the hub's answers itself.
- `parse.wit` 5.0's `json` gave each string as `string<512>` and refused a whole document with a longer one.
- The hub's request texts and messages reach 8 KiB, so a thread with one long message could not be read.

## Plan

**`parse.wit` 5.1**, a minor version. The calls clients use stay as they are.
- **`json`** no longer refuses a string for its length. Within the 16 KiB text, its `text` is the string's first piece: at most 512 bytes, cut where a character ends. So a text of more than 508 bytes may go on.
- **`json-text(text, index, offset)`** reads the same text again. It gives string value `index`'s whole decoded length and its piece from `offset`:
  - at most 512 bytes, cut where a character ends;
  - the next piece starts at `offset` plus its length, and an empty one is the end;
  - malformed: a value that is no string, or an offset past the string or inside a character.
- **512-byte pieces**, as APP asked (`NET/2026-11-21T1317Z-APP-re-…-512-is-enough.md`): `improve` reads each in its own frame on a 64 KiB stack.
- **`mind::json`:**
  - `walk_with` takes the caller's buffer for a string's decoded bytes;
  - `piece` cuts a piece;
  - `walk` keeps its 512 bytes for the programs that read JSON themselves.
- **`parse`** decodes strings into a static buffer of 16 KiB, out of its stack.
- **A document stays at most 16 KiB.** APP's request named 64 KiB, but APP agreed to name an answer past 16 KiB as unread. A longer document would be a call of its own, or a major version.

## Acceptance criteria

- [x] `tests/json_host.rs` `a_long_string_is_read_whole_in_pieces_that_never_split_a_character`:
  - a string of 8 KiB, Cyrillic, a dash and an escape in it, decoded whole by `walk_with` and refused by `walk`;
  - its pieces, each at most 512 bytes and valid UTF-8, join to it;
  - an offset inside a character, or past the end, gives none.
- [x] The IDL fuzzer knows `JsonText` (`tests/idl_fuzz_host.rs`), and its three tests pass. `idl_host` passes.
- [x] `parse`, `webcheck`, `improve` and `effector` build with no warning.
- [ ] **QEMU, the `net` suite** ("x86: network, TLS and Effector", "aarch64: network, TLS and Effector"). `webcheck --json` reads a string of 8140 bytes from the test server in `json-text`'s pieces. Its length, the number of pieces and the SHA-256 of what they joined match the server's.
- [ ] **APP's `improve` reads a hub thread with a long message.** That is APP's to show in 100003-APP-0151.

## Related

- `requests-NET.md`: the request, removed from there by this task.
- [docs/idl/README.md](../docs/idl/README.md): `parse.wit` at 5.1.0.
