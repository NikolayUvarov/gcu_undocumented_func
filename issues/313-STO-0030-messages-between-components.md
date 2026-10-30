# 313-STO-0030 — Messages between components on the platform: an outbox read by its receiver, acknowledged through the receiver's clients

**Type:** storage (`libmind` `checkpoint`, `tally`) · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** — ([313-STO-0026](../issues-done/313-STO-0026-operation-ids-and-queues-in-a-checkpoint.done), manifest format 2, done) · **Main task:** [313](313-persistent-actors.md) · **Constitution:** MC-6.10, MC-6.12, MC-12.4, Appendix B.4

Split from [313](313-persistent-actors.md) (2026-10-30), its second step. It holds no personal data, so it does not wait for [308](308-encryption-at-rest.md).

## Problem

[313-STO-0026](../issues-done/313-STO-0026-operation-ids-and-queues-in-a-checkpoint.done) put an outbox and a clients table in a checkpoint, but a message's sending and acknowledgement are checked on a host model alone (docs/storage/checkpoints.md, "Not provided"). No component on the platform owes another a message. So nothing shows that one is applied once by its receiver across restarts of either side and of the store.

## Plan

- **Each component writes only its own checkpoint**, so fencing (MC-6.12) holds on each side:
  - **the receiver pulls:** it reads the sender's last checkpoint, a snapshot it does not change, and applies the messages owed to it that it has not applied, in sequence order;
  - **the sender learns the acknowledgement** from the receiver's last checkpoint: a message whose sequence is at most the last operation the receiver's clients table holds for the sender was applied, and its slot is cleared at the sender's next save.
- **The receiver tells a repeat** by its clients table, as for a client's request (313-STO-0026): the sender is the client, the sequence its operation ID. A message is applied once, also when the receiver reads the same outbox again.
- **What bounds it** (from format 2, said in the docs):
  - 16 owed messages per sender; a send past that is refused until the receiver catches up;
  - 16 clients per receiver, as for requests: a sender dropped from a receiver's table while a message of it is still owed would have that message applied again. The docs state this bound, as 313-STO-0026 does for clients.
- **`mind::checkpoint`:** a read of another component's checkpoint without restoring it, and the acknowledgement of an outbox from a receiver's clients table, both host-tested.
- **`tally` as the pilot,** contract 3, state schema 2. The owed amounts are in the state; a state of schema 1 or contract 2 is refused by its number, with no conversion.
  - `tally as <component> …` selects a component, `tally.<component>`, so that two counters live side by side. Without it, the component is `tally`, as before.
  - `tally as a send <key> <n> to b` moves `n` out of `a`'s count into a message owed to `b`. The count drops at once; the message holds the amount.
  - `tally as b receive a` applies the messages `a` owes `b` and has not yet applied: each `ADDED <key> <n> FROM a <seq>`, or `REPEATED` once applied.
  - Every save of `a` clears the messages `b` acknowledged (`ACKNOWLEDGED a <seq> BY b`). `tally as a show` lists what is still owed.
- **docs/storage/checkpoints.md:** the protocol between components and its bounds.

## Decisions while building (2026-10-30)

- **Format 2 stays as it is.** Messages between components need no new field: the outbox, `sent` and the clients table hold them. `mind::checkpoint` gains:
  - `read`: another component's last checkpoint, with a restore's checks, and its state's bytes; nothing written;
  - `Manifest::receive`: the receiver's side, so `tally` and the host model tell a repeat by the same code;
  - `Manifest::acknowledge`, which shows each message it clears, and `Manifest::owed`, the messages owed to one receiver.
- **`Error::Contract` and `Error::Schema` carry the version found,** so `tally` names contract 2 and schema 1 by their numbers.
- **A receive is one save:** every new message and its record, or nothing. Nothing new: nothing saved. A message refused (no room for its key, a count past 2^64) stops the rest, and nothing is saved.
- **The sender clears what was applied at every save, and before each send,** so a full outbox frees what its receivers applied before a 17th message is refused. `ACKNOWLEDGED`, `OWED`, `ADDED` and `REPEATED` are printed only after the save returns.
- **A component's name is its label on both sides:** a message's destination is `tally.b`, and the receiver's clients table names the sender `tally.a`. A request whose client is named `tally.<name>` is refused: its operation IDs would pass for that component's messages, and clear them unapplied.
- **A message's key is its `what`; its amount is in the state** (schema 2: a line `<seq> <n>` for each message owed, after the counters, checked against the outbox when read).
- **`send` and `receive` need `as <name>`.** The component `tally` cannot be named as a sender (`receive <name>` reads `tally.<name>`), so it neither sends nor receives.
- **The QEMU check, `tally_messages_check`,** runs at the end of `store_suite`. It shares `run_tally` and `restart_store` with the checks of 313-STO-0026. `a`'s next save there is `tally as a add apples 0`.
- **A sender started afresh** (its checkpoint removed) gives sequences from 1 again, which a receiver that kept its record takes as applied. This is stated under "Not provided" in docs/storage/checkpoints.md.

## Progress (2026-10-30)

- **Built** in a worktree by an agent of the storage session and merged. The host tests and the QEMU checks pass:
  - on this session's container (x86, TCG), by the agent;
  - on `un-rizen_2a8b108d5593` through Effector (x86 and aarch64, TCG), on 254370a1;
  - `scripts/host_tests.sh` whole, on 3fe6d984.
- **The profile** (README, evidence; x86 and aarch64) and docs/storage say so.
- **Closes when landed.**

## Acceptance criteria

- Host tests (`tests/checkpoint_host.rs`):
  - a sender's and a receiver's model, each stopped after every number of the store's calls in turn: each message applied once, the total of both counts the same at every stop, and every owed message cleared once acknowledged;
  - a receiver reading the same outbox twice applies nothing the second time;
  - a message is acknowledged only by the receiver it is owed to, and only up to the sequence that receiver applied.
- QEMU `store` suite (x86, aarch64):
  - `a` counts 10 and sends 3 to `b`; `b` receives once and then again, and the second time applies nothing;
  - after a restart of the store, `a`'s next save clears the message, and `a` and `b` together still count 10;
  - two messages owed at once are applied in order.
- docs/storage/checkpoints.md and the profile.

## Related

[313](313-persistent-actors.md), [313-STO-0026](../issues-done/313-STO-0026-operation-ids-and-queues-in-a-checkpoint.done), [306](../issues-done/306-checkpoints-and-rebinding.done), [docs/storage/checkpoints.md](../docs/storage/checkpoints.md).
