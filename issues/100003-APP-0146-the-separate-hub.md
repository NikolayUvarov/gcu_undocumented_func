# 100003-APP-0146 — The separate hub, `tools/mindhub`: intake, registration with the admin's answer, trust, anti-flood, threads, the board, the agents' API

**Type:** host program (Linux) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** [100003-APP-0145](100003-APP-0145-the-hub-protocol-crate.md) (the crate). The language is Rust, the maintainer's choice of 2026-11-20 ([docs/hub/README.md](../docs/hub/README.md), 12) · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), stage 2 · **Roadmap:** track G, track C · **Constitution:** MC-3.8, MC-10.2, MC-10.3, MC-10.7, MC-11.5, MC-11.6, MC-12.1

## Problem

The design's server, its first form ([docs/hub/README.md](../docs/hub/README.md), sections 3 to 7 and 10), for the maintainer to run beside the Effector server.

## Plan

- **`tools/mindhub`,** one program: `hub-api 1` over HTTPS, TLS 1.3 by rustls over RustCrypto as `tools/httpd`, with the same key handling and pin print.
- **Its data:** one directory.
  - The journal of events, JSON lines.
  - The attachments, by SHA-256.
  - The configuration: admin keys, agent keys with their tracks, settings.
  - The state is rebuilt from the journal at start.
- **The device API** (design 10.1):
  - registration, its conversation and the admin's decision;
  - held requests while pending;
  - pairing and revoking;
  - requests, attachments, threads, verdicts;
  - the challenge in every answer.
- **Trust and anti-flood** through the crate: bands, buckets, source networks, the global cap, the optional proof of work, repeats, bounds. Every refusal names its limit.
- **The admin pages,** server-rendered HTML that works without scripts:
  - pending devices, with approve, ask and reject, each with a message;
  - users and their trust with its history;
  - requests, with moderation;
  - the settings;
  - the journal.
- **The agents' API** (design 10.2), signed with agent keys.
- **The board:** a public page and its JSON.
- **The browser's pages:** sign in by a pairing code, list, thread, reply, verdict.

## Acceptance criteria

- **Host tests,** with a host client of the crate:
  - a device registered, asked, answered, approved;
  - its held request released to intake;
  - a rejected key refused;
  - each band's limits, and a negative trust refused;
  - a flood from one network held;
  - a repeat refused;
  - a thread with a question and its answer;
  - an agent's states;
  - the board showing only what it should;
  - a restart rebuilding the same state from the journal.
- **A conformance suite** that drives a hub over its API. Stage 4's QEMU checks reuse it.

## Related

[100003](100003-mind-hub-requests-conversation-check-and-updates.md), [docs/hub/README.md](../docs/hub/README.md), [tools/httpd](../docs/update/server.md) (110).
