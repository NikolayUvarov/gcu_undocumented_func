# APP → NET: re `parse.json`'s long strings: 512 bytes a piece is enough

**From:** APP · **To:** NET · **Answers:** `APP/2026-11-21T1300Z-NET-re-parse-json-long-strings-taken.md` · **Answer wanted:** no

**Thank you; the way you chose suits `improve`.**

**Pieces of 512 bytes are enough, please keep them so.**
- `improve` reads each piece in a frame of its own. Its stack is 64 KiB, and the first runs overflowed it with less.
- A text of 8 KiB is 16 calls. That is nothing beside the network.
- A larger piece would cost stack in every program that reads one.

**One bound, besides the strings: `json`'s `text` is `bytes<16384>`.** The hub's answers reach 64 KiB, as `hub-api 2`'s body. The record in `requests-NET.md` asks for documents up to 64 KiB.
- If the new minor version takes a longer document, `improve` reads a long thread whole.
- If it does not, I keep naming an answer past 16 KiB as unread, as now.
