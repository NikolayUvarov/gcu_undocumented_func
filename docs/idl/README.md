# MIND IDL v0.2

**Version:** 0.2 (2026-10-04) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-2.3, MC-2.4, MC-2.12, MC-12.4, Appendix B.2 · **Roadmap:** [C5](../../ROADMAP.md)

MIND IDL describes the interface of a service: functions, the types and size limits of their data, the capability a call may carry and the version. Interfaces are written in a subset of [WIT](https://component-model.bytecodealliance.org/design/wit.html), the baseline candidate named in Appendix B.2. `scripts/mind_idl.py` generates Rust bindings for client and server into `libmind/src/idl/`. The generated files are committed, and `tests/idl_test.py` fails if they are out of date.

v0.2 covers the data representation and the acceptance of a single request: integers in two message words, and records, strings and lists in a memory buffer. Message ordering, session state machines and composition (MC-2.10) are not part of it yet.

## Syntax

```wit
/// Documentation comments are copied into the bindings.
package mind:rtc@1.0.0;

interface rtc {
    /// Seconds since midnight; none if the RTC cannot be read.
    now: func() -> option<u32>;
}
```

- One `package <namespace>:<name>@<major>.<minor>.<patch>;` and one `interface` per file `idl/<name>.wit`. The major version is 1–255.
- Functions: `name: func(params) -> result;`. There are 1–255 functions per interface, numbered in declaration order starting at 1.
- Parameter types:
  - `bool`, `u8`, `u16`, `u32`, `u64`;
  - `string<N>`: UTF-8 text of at most N bytes;
  - `list<T, N>`: at most N items; T is an integer, `string<N>` or a record;
  - records declared in the interface: `record name { field: type, ... }` (one or several lines), with the same field types;
  - at most one capability: `own<memory>`, `own<endpoint>` (moved, `CAP_TRANSFER_MOVE`) or `borrow<memory>`, `borrow<endpoint>` (copied, so the client can revoke it: a lease).
- Result: none; any of the data types above; `option<T>`; `result<T, error-code>` or `result<_, error-code>` (a system error code, `ERR_*`).
- **Bounds are part of the type.** `string<N>` and `list<T, N>` are a MIND extension of WIT syntax: every value has a static maximum size, so every buffer is bounded.
- Anything else is rejected by the generator: resources, variants, unbounded strings or lists, several capabilities, a capability in a function that also uses a buffer, more than 64 KiB per request or reply.

## Wire format

A function whose parameters and result are integers only (and at most one capability) is a **word call**: the two data words and the optional capability of an IPC `CALL`.

| Word | Request | Reply |
|---|---|---|
| 0 | bits 0–7 method, 8–15 major version, 16–63 fields | bits 0–7 status, 16–63 result |
| 1 | fields | result (if it does not fit in word 0), or the error code with status 2 |

Fields are placed in declaration order. A field never straddles a word boundary, so the size limit of a word call is 48 + 64 bits.

A function with a string, list or record anywhere is a **buffer call**:
- The client encodes the parameters into a buffer of its own, at least as large as the declared maxima of request and reply. It sends `method | major << 8 | request length << 16` in word 0 and 0 in word 1, with the buffer as a (copied) memory capability.
- The server writes the result into the same buffer and replies `status | reply length << 16`.
- Before reading the reply, the client revokes the server's copy, which unmaps it everywhere (MC-2.6). The server cannot change the reply while it is being decoded.

Encoding in the buffer, in declaration order, byte-packed, little-endian: integers in their size (`bool` one byte, 0 or 1), a string as a u16 byte length and the UTF-8 bytes, a list as a u16 count and the items, a record as its fields.

Status values:
- 0: ok;
- 1: `none` for an option result;
- 2: error of a `result<_, error-code>` (code in word 1);
- 0x80: the request failed the receiver's schema check;
- 0x81: the receiver serves another major version.

## Receiver checks (MC-2.4, MC-2.11, MC-2.12)

The generated `decode(request, cap_slot)` returns the request and a `Call` that the reply functions consume. It accepts a request only if all of the following hold:
- it is a call;
- the major version matches;
- the method exists.

For a word call, in addition:
- every bit outside the declared fields is zero;
- a capability is present exactly when the function declares one, and it is of the declared kind.

For a buffer call, in addition:
- a memory capability is present;
- word 1 is zero;
- the request length is within the declared maximum and the buffer;
- the buffer can hold the largest reply.

The request is then **copied into the server's private memory** before it is decoded (the client could change its buffer meanwhile, MC-2.11). Decoding checks every length against its bound, booleans, UTF-8, and that no bytes are left over.

A capability that is not accepted is dropped at once. The server answers a rejected request with `wire::reject`. A server may answer later: `Call::defer` keeps the right to reply (`IPC_SAVE_REPLY`) while it receives other requests. The kernel checks only rights and envelope limits; the schema check runs in the receiving service.

## Evolution (MC-12.4)

A change that alters the meaning or layout of an existing function increments the major version. Adding a function at the end increments the minor version: old clients never call it, and old servers answer it with 0x80. The minor and patch versions are not sent on the wire.

## Interfaces

| File | Service | Since |
|---|---|---|
| [`idl/rtc.wit`](../../idl/rtc.wit) | `rtc` | 1.0.0 |
| [`idl/tts.wit`](../../idl/tts.wit) | `tts` | 1.0.0 |
| [`idl/audio.wit`](../../idl/audio.wit) | `audio_gw` (`wait` is answered later, from the playback interrupt: `Call::defer`) | 1.0.0 |
| [`idl/block.wit`](../../idl/block.wit) | `ata`, `ahci`, `usb_storage` (client: `vfs_server`) | 1.0.0 |
| [`idl/vfs.wit`](../../idl/vfs.wit) | `vfs_server` (client: `mind::fs`; the write path and directory handles come with issue 048) | 1.0.0 |
| [`idl/init.wit`](../../idl/init.wit) | `init` (client: the shell's `RUN <service> &`) | 1.0.0 |
| [`idl/loader.wit`](../../idl/loader.wit) | `loader` (program list, start with arguments; the start with an endpoint for the child is a legacy adapter until loader v1, issue 046) | 1.0.0 |
