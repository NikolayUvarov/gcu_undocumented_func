# 100003-APP-0145 — The hub's protocol crate, `improve/proto`: forms, bounds, states, trust, buckets, signatures, vectors

**Type:** library (`no_std`, `alloc`) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), stage 1 · **Roadmap:** track G · **Constitution:** MC-3.8, MC-10.3, MC-11.5, MC-12.2

## Problem

The hub's two forms and its clients must agree byte for byte ([docs/hub/README.md](../docs/hub/README.md), sections 3 and 10):
- on the forms and bounds of a request and a message;
- on the state machine;
- on the trust bands and their token buckets;
- on what a signature covers.

Written three times, they would drift.

## Plan

- **`improve/proto`,** a crate for MIND Core's programs, the native hub, `tools/mindhub` and the host `improve`:
  - **the forms** of a device's registration, a request, an attachment's entry, a message of each kind, a state change, a board entry, with their JSON and their bounds (design 5.4);
  - **the state machine** of design 6.1: which role moves which state to which;
  - **the bands** of design 5.2, with the limits each gives, and a token bucket on a clock it is given;
  - the automatic trust changes and their bounds (5.3);
  - **the repeat check:** a request's normalised title and text;
  - **the signature's bytes:** the method, the path, the body's SHA-256 and the challenge, verified with Ed25519 (the crate the project already uses for releases);
  - the proof of work's check (5.4).
- **Test vectors** as files: signatures, buckets over time, every state transition allowed and refused. A hub in another language passes them (design 10.3).
- **Third-party code:** none new. The Ed25519 and SHA-256 crates are those the updater already uses ([THIRD_PARTY.md](../THIRD_PARTY.md)).

## Acceptance criteria

- **Host tests:**
  - each bound at its limit and one past it;
  - each band's bucket over a simulated day;
  - every state transition by every role, allowed or refused;
  - the automatic trust bounds;
  - a repeat;
  - signatures good, over a stale challenge and over a changed body;
  - the proof of work.
- **The vectors** are checked by the tests that make them.
- **`improve/proto` builds** for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat` as well as the host.

## Related

[100003](100003-mind-hub-requests-conversation-check-and-updates.md), [docs/hub/README.md](../docs/hub/README.md), [100003-APP-0146](100003-APP-0146-the-separate-hub.md).
