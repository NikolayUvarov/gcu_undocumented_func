# 038 — MIND IDL v0.2: records, strings and lists in a buffer

**Type:** architecture · **Priority:** P0 · **Status:** open · **Blocked by:** — · **Roadmap:** C5 follow-up, tools plan F6 · **Constitution:** MC-2.3, MC-2.4, MC-2.6, MC-12.4

## Problem

IDL v0 carries two words and one capability: no strings, records or lists. Paths, directory listings, task tables and launch requests do not fit.

## Plan

- `record` declarations with integer, `bool` and `string` fields (`string` has a declared maximum: `string<255>`).
- Bulk data in a memory buffer passed with the call: a function may take `buffer: borrow<memory>` and declare `in` and `out` payloads, e.g. `list: func(dir: u32, buffer: borrow<memory>) -> payload<list<entry, 128>>`. The two words carry method, version and payload lengths; the payload layout is little-endian, fixed-size fields, strings as `u16 length + bytes`, lists as `u32 count + items`.
- The generator emits `encode`/`decode` for records and lists with bounds checks; the receiver rejects a payload that is too long, truncated, not UTF-8 where a string is declared, or longer than the buffer (MC-2.4).
- Minor version rule unchanged; `rtc.wit` 1.1 adds `date: func() -> option<u32>` (days since 2000-01-01).

## Acceptance criteria

- `tests/idl_test.py` covers records, strings, lists, overflow and malformed payloads; generated Rust is fresh.
- Host round-trip test of generated encoders/decoders.

## Related

[docs/idl](../docs/idl/README.md), [docs/tools](../docs/tools/README.md) F6.
