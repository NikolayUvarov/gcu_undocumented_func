# MIND IDL v0.2

**Version:** 0.2 (2026-10-04; 0.1 the same day) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-2.3, MC-2.4, MC-2.12, MC-12.4, Appendix B.2 · **Roadmap:** [C5](../../ROADMAP.md)

MIND IDL describes the interface of a service: functions, the types and size limits of their data, the capability a call may carry and the version. Interfaces are written in a subset of [WIT](https://component-model.bytecodealliance.org/design/wit.html), the baseline candidate named in Appendix B.2. `scripts/mind_idl.py` generates Rust bindings for client and server into `libmind/src/idl/`. The generated files are committed, and `tests/idl_test.py` fails if they are out of date.

v0.2 covers the data representation and acceptance of a single request: scalars in the two message words and, since 0.2, strings, bytes, lists and records in a memory buffer lent with the call ([issue 038](../../issues-done/038-mind-idl-v0.2.done)). Message ordering, session state machines and composition (MC-2.10) are not part of it yet.

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
  - at most one capability: `own<memory>`, `own<endpoint>` (moved, `CAP_TRANSFER_MOVE`) or `borrow<memory>`, `borrow<endpoint>` (copied, so the client can revoke it: a lease).
- Result: none, an integer type, `option<integer>`, a bulk type (below) or `result<T, E>` where `T` is `_` (no value), a scalar or a bulk type and `E` is an enum.
- Since 0.2, inside the interface:
  - `enum name { a, b-c }` — 1..255 cases, carried as `u8`; an enum may be a parameter, a result, a record field, a list item or the error of `result<>`;
  - `record name { field: type, ... }` — fields are integers, `bool`, enums, `string<N>`, `bytes<N>` or records declared earlier; no lists inside records.
- Bulk types (since 0.2): `string<N>` (UTF-8, at most N bytes, N ≤ 65535), `bytes<N>` (N ≤ 1 MiB), `list<T, N>` (T an integer, enum or record; N ≤ 4096), records. A function that has bulk parameters or a bulk result must take exactly one `borrow<memory>` parameter: the buffer the bulk data travels in.
- Anything else (variants, resources, lists of strings, several capabilities) is rejected by the generator, as are the reserved names `request`, `reject`, `status`, `buffer`, `result`.

## Wire format

A message is the two data words and the optional capability of an IPC `CALL`.

| Word | Request | Reply |
|---|---|---|
| 0 | bits 0–7 method, 8–15 major version, 16–63 fields | bits 0–7 status, 16–63 result |
| 1 | fields | result (if it does not fit in word 0) |

Fields are placed in declaration order. A field never straddles a word boundary, so the size limit of a call is 48 + 64 bits; the generator rejects interfaces that do not fit. A function with bulk parameters has an implicit first field `payload: u32`, the length of its encoded bulk arguments. Status values:
- 0: ok; a bulk result's encoded length is in bits 16..48 of word 0;
- 1: `none` for an option result;
- 2: the error of a `result<T, E>`: the enum value is in bits 16..24;
- 3: the result does not fit the client's buffer (the client sees `ERR_LIMIT`);
- 0x80: the request failed the receiver's schema check;
- 0x81: the receiver serves another major version.

### Bulk data (0.2)

The client writes its bulk arguments, in declaration order, at the start of the buffer it lends (`borrow<memory>`, so it can revoke it), and the server writes a bulk result at the start of the same buffer. Encoding: integers little-endian at their width, `bool` as 0 or 1, enums as `u8`, `string<N>` as `u16` length + UTF-8, `bytes<N>` as `u32` length + bytes, `list<T, N>` as `u32` count + items, records as their fields in order. Nothing is aligned or padded.

## Receiver checks (MC-2.4, MC-2.12)

The generated `decode(request, cap_slot)` accepts a request only if all of the following hold:
- it is a call;
- the major version matches;
- the method exists;
- every bit outside the declared fields is zero;
- a capability is present exactly when the function declares one, and it is of the declared kind (`CAP_INFO`);
- every enum value in the words is a declared case.

Bulk arguments are checked by the generated `args_<function>(mapped buffer, payload)` once the server has mapped the buffer: the payload fits the buffer, every string is UTF-8 within its limit, every list within its count, every `bool` 0 or 1, every enum a declared case, and the payload is consumed exactly (no trailing bytes). The client checks a bulk result the same way (and that its length fits the buffer) before it returns a `wire::List` view or records that borrow from the buffer.

A capability that is not accepted is dropped at once. The server answers a rejected request with `wire::reject`. The kernel checks only rights and envelope limits; the schema check runs in the receiving service.

## Evolution (MC-12.4)

A change that alters the meaning or layout of an existing function increments the major version. Adding a function at the end increments the minor version: old clients never call it, and old servers answer it with 0x80. The minor and patch versions are not sent on the wire.

## Interfaces

| File | Service | Since |
|---|---|---|
| [`idl/rtc.wit`](../../idl/rtc.wit) | `rtc` | 1.1.0 (`date` added) |
| [`idl/sysinfo.wit`](../../idl/sysinfo.wit) | `sysmon` | 1.0.0 |
| [`idl/loader.wit`](../../idl/loader.wit) | `loader` (launch sessions) | 1.0.0 |
| [`idl/vfs.wit`](../../idl/vfs.wit) | `vfs_server` | 2.0.0 (replaces the numeric VFS protocol) |

`tests/idl/sample.wit` exercises every v0.2 type; its bindings (`tests/idl/sample.rs`) run in `tests/idl_host.rs` against a loopback of client and server, with malformed payloads.

The other services (block, audio, TTS, init) and the loader's `LOADER_RUN`/`LOADER_LIST` still use the numeric conventions in `common/abi.rs`; moving them to MIND IDL is roadmap C8.
