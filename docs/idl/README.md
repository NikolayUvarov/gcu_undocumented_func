# MIND IDL v0

**Version:** 0.1 (2026-10-04) · **Constitution:** [v1.6](../../constitution/EN/MIND_CORE_Constitution_v1.6.md) MC-2.3, MC-2.4, MC-2.12, MC-12.4, Appendix B.2 · **Roadmap:** [C5](../../ROADMAP.md)

MIND IDL describes the interface of a service: functions, the types and size limits of their data, the capability a call may carry and the version. Interfaces are written in a subset of [WIT](https://component-model.bytecodealliance.org/design/wit.html), the baseline candidate named in Appendix B.2. `scripts/mind_idl.py` generates Rust bindings for client and server into `libmind/src/idl/`. The generated files are committed, and `tests/idl_test.py` fails if they are out of date.

v0 covers the data representation and acceptance of a single request. Message ordering, session state machines and composition (MC-2.10) are not part of it yet.

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
- Result: none, an integer type or `option<integer>`.
- Anything else (records, strings, lists, resources, several capabilities) is rejected by the generator.

## Wire format

A message is the two data words and the optional capability of an IPC `CALL`.

| Word | Request | Reply |
|---|---|---|
| 0 | bits 0–7 method, 8–15 major version, 16–63 fields | bits 0–7 status, 16–63 result |
| 1 | fields | result (if it does not fit in word 0) |

Fields are placed in declaration order. A field never straddles a word boundary, so the size limit of a call is 48 + 64 bits; the generator rejects interfaces that do not fit. Status values:
- 0: ok;
- 1: `none` for an option result;
- 0x80: the request failed the receiver's schema check;
- 0x81: the receiver serves another major version.

## Receiver checks (MC-2.4, MC-2.12)

The generated `decode(request, cap_slot)` accepts a request only if all of the following hold:
- it is a call;
- the major version matches;
- the method exists;
- every bit outside the declared fields is zero;
- a capability is present exactly when the function declares one, and it is of the declared kind (`CAP_INFO`).

A capability that is not accepted is dropped at once. The server answers a rejected request with `wire::reject`. The kernel checks only rights and envelope limits; the schema check runs in the receiving service.

## Evolution (MC-12.4)

A change that alters the meaning or layout of an existing function increments the major version. Adding a function at the end increments the minor version: old clients never call it, and old servers answer it with 0x80. The minor and patch versions are not sent on the wire.

## Interfaces

| File | Service | Since |
|---|---|---|
| [`idl/rtc.wit`](../../idl/rtc.wit) | `rtc` | 1.0.0 |

The other services (VFS, block, audio, loader, TTS, init) still use the numeric conventions in `common/abi.rs`; moving them to MIND IDL is roadmap C8.
