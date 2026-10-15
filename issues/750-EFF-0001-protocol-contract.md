# 750-EFF-0001 — The Effector protocol subset the agent implements, as a versioned contract

**Type:** contract (documentation) · **Owner:** `EFF` track (open) · **Priority:** P1 · **Status:** open · **Blocked by:** — (publishing approved by the maintainer, 2026-10-09: [issues-human](../issues-human/README.md#7-an-effector-server-for-mind-core)) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-2.3, MC-12.4, MC-12.7, MC-12.3

## Problem

- **Original survey (2026-10-09): not published.** At that survey the agent routes were described only by the server and its agents. **Updated 2026-10-15:** the native protocol is now published at `/effector/agent-protocol` and included in the server OpenAPI. MIND still needs its own bounded, versioned subset and source attribution.
- **Implicit rules.** Several of its rules are implicit:
  - the online window;
  - the character set of an agent ID;
  - which actions an agent may refuse;
  - how a result string becomes an operation's final state.
- **Why that matters.** An agent written against such behaviour breaks silently when the server changes (MC-12.7).
- **Source.** The original survey used build 1.2.0.127 (`5d42b29`). The current contract records source `a29dd0086bf278e3e913f9b5203a70c96a4abcec` and the separately observed live build 1.2.0.214 (`a65552484d69`). The old external task 75 link is stale; that number now describes unrelated console work.

## Plan

- **Write `docs/effector/protocol.md`:** the subset the MIND Core agent implements, with a contract version (`effector-agent 1.0`) and the server builds it was read from.
- **For each route:** method and path, headers, the JSON fields the agent sends and the ones it reads, size bounds, status codes, and what the agent does on each error.
- **Timing:**
  - the heartbeat interval (one to two seconds) and the server's online window;
  - the stream's lifetime and the poll interval;
  - the ACK retry rule.
- **The action table:** every action the server can send, with its payload, the typed request it becomes (0003) or `denied`, and the result the agent returns. The result must map to the intended final state, for example `denied` and not `failed`.
- **Identity:**
  - an agent ID of `[A-Za-z0-9_-]`, 1–64 characters, with at least 64 random bits, kept in the agent's directory;
  - `os_version` `mindcore`;
  - `arch` `x64` or `arm64`;
  - the release version as `agent_version`.
- **Path grammar for files:** the MIND Core volume paths the agent accepts (`data/…`, `log:/…`), agreed with the server's side (its task 75).
- **Compatibility rules:**
  - unknown fields are ignored;
  - a missing required field is an error the agent reports;
  - a server contract the agent does not know is not guessed at;
  - a change to the subset is a new contract version.
- **What the subset leaves out, and why:** shell actions, the console, Windows identities and plain HTTP.
- **What stays out of this public document:** the server's internal weaknesses. They belong to the server's own project.

## Acceptance criteria

- `docs/effector/protocol.md` exists with the contract version and the server builds it covers. Every route and action the agent uses is there, and the 0002 test server and the 0006/0007 services cite it.
- Publishing this subset here was approved by the maintainer on 2026-10-09 (supporting MIND Core is a requirement on the server); the document says so and links that decision.

## Related

[750](750-effector-agent.md), [750-EFF-0002](750-EFF-0002-test-server.md), [750-EFF-0003](750-EFF-0003-interface-and-policy.md), [docs/idl/README.md](../docs/idl/README.md).

## Progress — 2026-10-15

- [Contract 1.0](../docs/effector/protocol.md) now specifies routes, token versus operator credentials, TLS/pins, the base path, identity, timing, error handling, action mapping, journal/ACK rules and bounds with exact source/live revisions.
- The [local fixture](../tests/effector_server.py) cites that contract; its 12 host tests pass on Linux. They are fixture evidence, not agent interoperability evidence.
- The runtime services do not exist yet, so their required contract citations and their use of the contract remain with 0006/0007. This issue stays open until that final acceptance item is met; no acceptance criterion was removed.
