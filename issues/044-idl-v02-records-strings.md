# 044 — MIND IDL v0.2: records, strings and lists in buffers (tools F6)

**Type:** kernel/IDL · **Owner:** kernel track (moved from the tools track: C8 needs it first) · **Priority:** P1 · **Status:** open · **Roadmap:** track G, C8

## Problem

IDL v0 carries two words; paths, directory entries and task tables do not fit.

## Plan

- `record`, `string`, `list<record>` with declared maxima, placed in a memory buffer passed with the call (`borrow<memory>` for results, sealed read-only memory for inputs); generated encoders/decoders with bounds checks; the receiver rejects malformed buffers.
- `docs/idl` v0.2; tests extend `tests/idl_test.py` (compile and round-trip).
- Bounds are part of the type (`string<N>`, `list<T, N>`), a MIND extension of WIT syntax. Inputs are copied into a private buffer before they are validated (MC-2.11); the client revokes the server's access to its buffer before reading the reply. Errors are `result<T, error-code>`.
- First user: `loader.wit` (program list, start with arguments); the legacy start-with-endpoint request stays as a bounded adapter until loader v1 (046).

## Acceptance criteria

- Generator tests incl. malformed buffers; one interface (e.g. `sysinfo.wit`) used end to end in QEMU.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F6; [docs/idl](../docs/idl/README.md).
