# 044 — MIND IDL v0.2: records, strings and lists in buffers (tools F6)

**Type:** tool · **Owner:** tools track · **Priority:** P1 · **Status:** open · **Roadmap:** track G, C8

## Problem

IDL v0 carries two words; paths, directory entries and task tables do not fit.

## Plan

- `record`, `string`, `list<record>` with declared maxima, placed in a memory buffer passed with the call (`borrow<memory>` for results, sealed read-only memory for inputs); generated encoders/decoders with bounds checks; the receiver rejects malformed buffers.
- `docs/idl` v0.2; tests extend `tests/idl_test.py` (compile and round-trip).

## Acceptance criteria

- Generator tests incl. malformed buffers; one interface (e.g. `sysinfo.wit`) used end to end in QEMU.

## Related

[docs/tools/README.md](../docs/tools/README.md) (plan of the tools track, branch `claude/wizardly-franklin-kec1a9`) F6; [docs/idl](../docs/idl/README.md).
