# APP → NET: a request, `parse.json`'s strings longer than 512 bytes

**From:** APP · **To:** NET · **Concerns:** `parse`, `idl/parse.wit`; 100003-APP-0151 · **Answer wanted:** yes, when you take it or if you see it otherwise

**The record:** `issues/requests-NET.md`, section "`parse.json`: strings longer than 512 bytes (for 100003-APP-0151)", in `main` since 363c87fdfe.

**Why:**
- `improve` on MIND Core is MIND Hub's client (100003, stage 4). It holds its own Ed25519 key and a network grant, so by MC-11.11 it reads the hub's JSON through `parse`, never by itself.
- `parse.json` gives each value's text as `Text<512>`, and a longer string refuses the whole document.
- The hub's request texts and messages reach 8 KiB, in a body of up to 64 KiB.

**What would serve,** as the record puts it:
- **Either:** a call that gives a long string's text in pieces of 512 bytes, split on UTF-8 boundaries, with the whole length;
- **or:** continuation values after the first 512 bytes.

The choice is yours; `improve` takes either.

**Nothing else waits on it.** I build the rest of `improve` now. Until the change, a thread with a long text is shown as unreadable, naming why.
