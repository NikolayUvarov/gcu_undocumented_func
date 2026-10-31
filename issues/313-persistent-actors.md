# 313 — Persistent actors: checkpoints of services with queues

**Type:** main task · **Owner:** `STO` track · **Priority:** P2 · **Status:** open · **Blocked by:** [308](308-encryption-at-rest.md) (for actors that hold personal data) · **Roadmap:** track B "checkpoint/rebind" · **Constitution:** MC-6.10, MC-6.11, MC-6.12, RFC 001 §9

From the kernel session's draft (`STO/2026-10-25T1557Z-KRN-draft-plan-the-store-as-a-minds-memory.md` on `internal-dev-messages`), taken by the storage session at the maintainer's word (2026-10-27). Everything here is plan, not a guarantee (MC-12.3).

## Problem

[306](../issues-done/306-checkpoints-and-rebinding.done) gave checkpoints one pilot, `tally`. No service with queues checkpoints its state (docs/storage/checkpoints.md, "Not provided"), so an actor that restarts loses its unfinished work and may repeat an effect.

## Plan

- **`mind::checkpoint` extended:** inbox and outbox offsets and operation IDs; an effect journal (the intent before an effect, the result after); an indeterminate outcome reconciled, never blindly repeated; fencing by epoch; rebinding by the description of needs.
- **First users:** the agent module's sessions (260, `AGT`) and the messenger client's last event (950, `COM`). `tally` stays the pilot.
- **Identity:** an actor's identity apart from its instance's generation and place (MC-6.12); "who is active" held as a name with compare-and-swap, so two instances never both take commands; a move to another machine through a copy of its store ([309](309-copies-scrub-and-repair.md)) and a rebind.

## Tasks

- [313-STO-0026](../issues-done/313-STO-0026-operation-ids-and-queues-in-a-checkpoint.done): operation IDs and queues in a checkpoint, manifest format 2, `tally` as the pilot. Done 2026-10-30.
- [313-STO-0030](../issues-done/313-STO-0030-messages-between-components.done): messages between components on the platform, pulled by the receiver and acknowledged through its clients table; `tally` as the pilot. Done 2026-10-30.
- [313-STO-0033](../issues-done/313-STO-0033-a-component-moved-through-a-copy.done): a component moved to another store through a copy (309-STO-0031), released at the source so that it is active in one place only. Done 2026-10-30.

## Acceptance criteria

- Host: a stop at every point between an effect and its record, each reconciled.
- QEMU: a restarted agent resumes its session; one restored on another VM while the old one is fenced.

## Related

[306](../issues-done/306-checkpoints-and-rebinding.done), [309](309-copies-scrub-and-repair.md), [312](312-the-minds-memory.md), 260, 950.
