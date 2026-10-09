# 351-NET-0011 — The parser service reads release channels and boot manifests

**Type:** service (network) · **Owner:** `NET` track · **Priority:** P1 · **Status:** open · **Blocked by:** — ([351-UPD-0013](../issues-done/351-UPD-0013-release-metadata-in-libmind.done) is done) · **Main task:** [351](351-self-update.md), for [351-UPD-0007](351-UPD-0007-updater-service.md) · **Constitution:** MC-11.5, MC-11.11, Appendix B.6

## Problem

The updater must not parse the channel file and the manifests it fetches (MC-11.11). The parser service `parse` has to read them for it, as it reads `download`'s response heads ([109](../issues-done/109-session-parsers.done)).

## Plan

- `idl/parse.wit` 1.1 (a minor version: the HTTP head call is unchanged, MC-12.4):
  - `channel(file: bytes<1024>)`: the channel's fields, the length of the signed line and the signature;
  - `manifest(text: bytes<32768>, start: u32)`: the header lines, the number of files, and up to 32 file lines from `start`.
- `parse` serves them with `mind::release` and logs a refusal with the client's PID, as for heads.
- `mind::parse` gives the client side.

## Acceptance criteria

- Host tests: the records round-trip through the IDL.
- On QEMU, a client gets a channel and a manifest read by the service, and a malformed one refused and logged. The updater's check (that the answer encodes to the signed bytes) is 351-UPD-0007's.

## Related

[351-UPD-0013](../issues-done/351-UPD-0013-release-metadata-in-libmind.done), [109-NET-0008](../issues-done/109-NET-0008-parser-service.done).
