# 313-STO-0026 — Operation IDs and queues in a checkpoint: manifest format 2

**Type:** storage (`libmind` `checkpoint`, `tally`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [313](313-persistent-actors.md) · **Constitution:** MC-6.10, MC-6.12, MC-12.4, Appendix B.4

Split from [313](313-persistent-actors.md) (2026-10-30), its first step at the maintainer's word. It holds no personal data, so it does not wait for [308](308-encryption-at-rest.md).

## Problem

A checkpoint ([306](../issues-done/306-checkpoints-and-rebinding.done)) records one consistency point and an effect journal. Three things are missing for a service that takes requests and sends messages:
- **A request repeated after a restart is applied again.** A client whose reply was lost cannot tell whether its request was applied, and retries it.
- **An input queue's place is not kept.** A service that reads a numbered source starts over or skips.
- **A message the service owes another is lost** when it ends between deciding to send and sending.

## Plan

- **Manifest format 2.** Format 1 is refused by its number (`Error::Version(1)`), with no conversion: a store that holds one is written again by hand. It adds three things:
  - **Clients** (at most 16): for each client (a label), the last operation ID applied and its result (a u64), and the sequence at which it was applied.
    - A request carries its client's operation ID, increasing.
    - An ID above the last one is new. One equal to it is a repeat, answered with the recorded result and never applied again. One below is stale and refused.
    - When all 16 are taken, the client applied longest ago is dropped. Repeats are recognized only for the 16 most recent clients, and that is said.
  - **The inbox:** how far an input queue was consumed (a u64 the source numbers).
  - **The outbox** (at most 16): messages the service owes others, each a sequence number, a destination and what it is (labels; the message's bytes are in the state).
    - After a restore, each is sent again until its receiver acknowledges it.
    - A receiver tells a repeat by the sender and the sequence, as its own clients table does.
    - Unlike an effect, a message needs no reconciling: the receiver makes it idempotent.
- **The order of a request,** as for `tally` now:
  1. apply it to the state in memory, with its client's record;
  2. put the state;
  3. commit the checkpoint;
  4. answer.

  The answer and any message go out only after the commit.
- **Who is active:** an instance saves once before it takes its first request (`claim`). An instance restored before that save is fenced at its own first save (MC-6.12).
- **`tally` as the pilot:**
  - `tally add <key> [n] [op <client> <id>]` counts a repeated operation once, and says it is a repeat;
  - its contract becomes version 2.

## Decisions while building (2026-10-30)

- **The claim is a save,** with nothing changed, before the first request: no function of its own.
- **The five counts stand together** after the state's CID, then `inbox` and `sent`, then the lists in order. A manifest of format 2 thus starts as format 1 did, up to its counts.
- **`sent` is kept apart from the outbox,** so a sequence is never given twice, even after every message was acknowledged.
- **An acknowledged message's slot is cleared,** so two manifests that encode alike compare alike.
- **`tally`'s contract is version 2.** A repeat prints `REPEATED <client> <id>: ANSWERED <count>, NOT APPLIED AGAIN`; an older ID is refused; an addition without an ID is applied each time, as before. `tally` takes no messages, so the outbox is checked on the host model alone.

## Acceptance criteria

- Host tests (`tests/checkpoint_host.rs`):
  - format 2 encodes and decodes to the same manifest, and only its canonical bytes decode;
  - format 1 is refused by its number;
  - a client's new, repeated and stale operations, and the eviction of the oldest client;
  - a model of a service that stops at every point between a request and its answer: each operation applied once; a repeat answered with the first result; no acknowledged request lost; each outbox message sent until acknowledged, and received once;
  - `claim` fences an instance restored before it.
- QEMU `store` suite (x86, aarch64): `tally add` with the same operation twice counts once, and says so; the count survives a restart.

## Related

[313](313-persistent-actors.md), [306](../issues-done/306-checkpoints-and-rebinding.done), [docs/storage/checkpoints.md](../docs/storage/checkpoints.md).
