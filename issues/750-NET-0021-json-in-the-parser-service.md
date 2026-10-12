# 750-NET-0021 — JSON in the parser service

**Type:** network (service, interface, library) · **Owner:** `NET` track (worked by the storage session at the maintainer's request) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) (with 501) · **Roadmap:** track D · **Constitution:** MC-11.5, MC-11.11, Appendix B.6

The request in requests-NET.md ("JSON in the parser service", 2026-10-09).

## Problem

Effector's commands and replies are JSON. Under MC-11.11 the process that holds the server's token, the flow and the TLS client does not parse them; 109's `parse` reads HTTP heads, release metadata and model manifests only.

## Plan

- **`mind::json::walk`:** a whole JSON text (RFC 8259) read with `mind::json`'s reader into its values in document order. Each value comes with its kind, its parent's index, its member's key, and its text (a string decoded, a number as written). The bounds:
  - 1024 values;
  - nesting 16 deep;
  - a key of 64 bytes and a string of 512 bytes once decoded.

  Anything past them is malformed.
- **`idl/parse.wit` 1.4 `json`:** the text (up to 16 KiB) in, the values out 32 at a time from `start`, and how many there are. A refusal is logged with the client's PID.
- **The client still checks the tree against what it asked for (MC-11.5).** A key that comes twice is the client's to refuse.
- **The library task of the Effector track** (750-EFF-0004, `mind::json` with a writer and the JSONTestSuite) is not taken here. This builds on the reader `mind::json` already has (251-STO-0014).

## Acceptance criteria

- **Host tests** (`tests/json_host.rs`): a command in document order; malformed texts refused; every bound reached and passed.
- **In QEMU** (the `net` suite's download check, x86 and aarch64), with `webcheck --json`:
  - `parse` turns a JSON answer and a command into their values;
  - it refuses 20 levels of nesting and logs the refusal with the client's PID.
- **The authority map** gains the agent's stage when 750-EFF-0006 lands (the request's last point, not this task's).

## Related

[docs/network/airlock.md](../docs/network/airlock.md), [109-NET-0008](../issues-done/109-NET-0008-parser-service.done), [750-EFF-0004](750-EFF-0004-json.md).
