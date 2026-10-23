# MIND Core — The Substrate and Its Boundaries

**Edition:** 1.6 · **Date:** 2026-10-23

**Status:** current non-normative architectural and cultural canon

**Supersedes:** [LORE 1.5](archive/LORE_v1.5.EN.md) · **Other language:** [Русский](LORE.RU.md)

This document does not amend the Constitution, assign new capabilities or claim implementation conformance. Only the Constitution's articles are normative.

**Related documents:** [Constitution 1.6](../EN/MIND_CORE_Constitution_v1.6.md); [Marain RFC 001 v0.4](../EN/RFC_001_Marain_v0.4.md); [platform profile](../../docs/profile/README.md); [cultural-code task 850](../../issues/850-the-cultural-code.md). **Evidence snapshot:** `main` `bb7262bd` (2026-10-23); statuses below describe repository records at that revision, not a fresh execution of tests.

## 0. A world with explicit boundaries

A program should have the authority its work needs. When authority arrives merely because a program belongs to a powerful user or happens to share a trusted component, the consequences of one error can extend far beyond its task. MIND Core explores a substrate in which authority, state, resources and recovery have explicit contracts.

The cultural image of its adversary is **hidden power**: a dependency no one named, a right no one meant to grant, an unbounded queue, an effect whose outcome was quietly guessed. This is a description of architectural failure modes, not a claim that every other operating system inevitably fails. Existing systems and research remain sources of useful mechanisms and evidence.

This canon explains the system's boundaries for people and autonomous agents. The manifesto's protagonist remains a separate editorial decision under task 850. The Constitution neither establishes the consciousness of a model nor grants intelligence a privileged bypass. Its preamble defines a substrate for an autonomous Mind. MC-8.1 and MC-12.6 keep the Mind within the authority model.

Only the Constitution's articles are normative. This canon explains values and vocabulary. RFCs describe mechanisms under development. Platform profiles state what is and is not implemented, under which assumptions. An image of the future never substitutes for such a profile.

## I. Ontology — identity, boundary and cost

**Maxim: “A published version has an identity; access has a boundary; work has a budget.”**

A protection domain is an enforced boundary of authority and state. An actor is a unit of computation; several actors may share one domain, and their runtime determines what isolation exists between them. The kernel is part of the trusted computing base, whose full composition depends on the guarantee being discussed (MC-1.6, MC-2.1–2.2).

Published native objects have immutable representations. A stable logical entity, an exact version and the authorized reference to the current version are different things. Local mutable working state, caches and temporary buffers remain legitimate; not every intermediate thought becomes a stored object (MC-4.1–4.3, MC-4.9).

Sharing is an explicit contract: copy, exclusive move, immutable read, a bounded lease, or separately justified mutable sharing. A read-only mapping alone does not establish immutability; expiry alone does not establish that CPU or DMA access has ended (MC-2.6–2.8, MC-2.13). A universal identity-based bypass is excluded by MC-3.7.

| Enforced today, within the named profile | Not yet established |
|---|---|
| MC-2.1, MC-3.1–3.2: private address spaces, memory capabilities and kernel-managed generations are recorded in the [profile](../../docs/profile/README.md), with the `isolation` suite listed in its [evidence](../../docs/profile/evidence.md). Native content-addressed storage (MC-4.2–4.4) has its own limits and `store`/`storefaults` evidence. | Universal actor isolation, a fully native CAS-only environment, and every resource guarantee on every physical platform. FAT media coexist with native storage; actors are not synonymous with protection domains. |

## II. Epistemology — knowing what a claim establishes

**Maxim: “A hash identifies bytes. A source answers for a claim. Silence does not cancel an effect.”**

A digest identifies a representation under a declared format and hash algorithm. It does not establish semantic truth, freshness, confidentiality, permission to read, or absence of errors. A model inference, embedding or label is a claim by a source, with provenance and an interpretation version (MC-4.6–4.7, MC-4.12, MC-9.2).

Code identity, the build recipe, the produced artifact and authorization to execute are separate. Marain's proposed `CodeID`, `BuildKey` and `ArtifactCID` vocabulary makes those distinctions visible; these names do not make the whole pipeline implemented or prove that an artifact is safe to activate.

An external message remains an external claim after it has been parsed and typed. Stable parsing, authority checks and the executor's domain checks are distinct steps (MC-2.4, MC-2.11, MC-11.5). A capability is necessary for a protected action; the operation's contract supplies its other conditions.

An accepted request, a completed action and knowledge of its outcome are different. A timeout may leave `OutcomeUnknown`. Reconciliation establishes the old action's result separately from the resource's current controller; fencing must be checked where the resource actually accepts commands (MC-6.6, MC-6.12, MC-7.1, MC-7.4). Side-channel protection has its own scope and assumptions (MC-10.5).

| Enforced today, within the named profile | Not yet established |
|---|---|
| MC-4.2–4.4, MC-6.6, MC-6.10–6.12: content checks, authorized publication and the limited `tally` checkpoint pilot are documented in the [profile](../../docs/profile/README.md), `store`/`storefaults` suites and [checkpoint host checks](../../tests/checkpoint_host.rs). Boot provenance (MC-9.1–9.2) is partially implemented; see the profile's Article 9 row. | General reconciliation for all services and physical effects, universal checkpointing, complete distributed control, and the full Marain build identity pipeline. |

## III. Ethics — continuity within limits

**Maxim: “A fault has a boundary. Recovery has a budget.”**

The aim is continuity of critical functions within a declared model. The survival of a Mind does not authorize it to enlarge its powers, hide its costs or disregard physical limits (MC-5.5, MC-6.1, MC-8.1–8.4).

A component has a lifecycle owner, a failure recipient, bounded recovery and an escalation path. Restarting a process does not stop every device by itself, restore revoked rights or undo an action already performed (MC-6.3, MC-6.5, MC-6.8–6.11).

Mutable models and planners belong to the cognitive plane. Critical physical action belongs behind a separate control contract that checks authority, state, parameters, sequence and deadlines at execution. Typing a proposal does not prove physical admissibility. The safe failure mode depends on the controlled process; “turn everything off” is not a universal rule (MC-8.2–8.4).

| Enforced today, within the named profile | Not yet established |
|---|---|
| MC-6.1–6.2, MC-6.5: ring-3 fault containment, restart budgets and recovery reserves have recorded `isolation`, `services` and `memory` evidence in the [profile](../../docs/profile/README.md) and its [evidence index](../../docs/profile/evidence.md), with the stated implementation limits. | A complete safety plane or arbitrary-failure survival. The base profile explicitly marks Articles 7 and 8 `not claimed` ([profile, Article 8](../../docs/profile/README.md)); SAF is a later track. |

## IV. Praxeology — proposals, grants and change

**Maxim: “A proposal grants no authority.”**

Authority has provenance, scope and a lifecycle. Derived authority cannot exceed its parent; a separate authorized grant is a separate event. Delegation is not process kinship. Revocation has a declared scope and an observable completion boundary, and does not undo past actions or erase information already read. An unreachable remote node cannot be assumed to observe revocation instantly (MC-3.3–3.6, MC-7.5).

The kernel enforces the mechanisms assigned to it; application policy and physical admissibility remain with the responsible services. No compiler or adapter is a universal oracle that rejects every possible bad effect before it occurs (MC-1.4, MC-2.9–2.10, MC-6.7, MC-11.2).

Self-modification is permitted through verifiable procedures. Knowledge, executable code, configuration and authority are kept distinct. An ordinary optimization may change only the parameters and ranges its contract permits; changing the contract or its upper budgets requires separate authority (MC-8.5, MC-12.6).

| Enforced today, within the named profile | Not yet established |
|---|---|
| MC-3.4–3.6: capability minting, attenuation and local revocation are documented, with their scope, in the [profile](../../docs/profile/README.md). The `isolation` suite and bounded `CapRevokeMove`/`RevokeFlush` model checks are listed in [assurance](../../docs/assurance/README.md); they are not a proof of the whole implementation. | Instant global revocation, an implemented autonomous diagnosis-to-PR loop, or a general model service. Tasks 263, 260 and 262 describe those next steps. |

## V. Aesthetics — visible reasons for trust

**Maxim: “The contract shows the boundary. The profile names what must be trusted.”**

The ideal is a small set of comprehensible mechanisms, explicit effects and visible dependencies. Types help express contracts; runtime authorization, input validation, hardware assumptions and domain checks still matter. Unsafe code and FFI are localized and accounted for, not wished away (MC-11.1–11.2, MC-11.7–11.8).

Content identity is separate from location, logical naming, access and retention. Trees and folders are allowed as views over objects; a useful interface need not expose every mechanism at all times (MC-4.1, MC-4.11–4.12).

| Enforced today, within the named profile | Not yet established |
|---|---|
| MC-1.1, MC-1.6: [850-KRN-0145](../../issues-done/850-KRN-0145-a-budget-for-the-kernels-size.done) records the kernel-size budget in `scripts/host_tests.sh`; [TCB](../../docs/profile/tcb.md) names the omitted compiled files. [850-KRN-0146](../../issues-done/850-KRN-0146-refusals-name-the-clause.done) records selected clause-linked refusals and the `modes` check (MC-12.6); it deliberately avoids logging every refused syscall (MC-10.2). Other listed refusal lines lack hardware tests. | Security inferred from a line count, a full system topology UI, complete absence of unsafe code, or a constitutional log record for every failure. |

## VI. The lexicon: technical roles and cultural names

| Word | Meaning and limit |
|---|---|
| Substrate | The computational basis described by the Constitution; not a synonym for the kernel alone. |
| Kernel | Privileged boundary mechanisms; part of the TCB for the guarantees that depend on it. |
| Domain / actor | Enforced protection boundary / computation unit; the distinction is intentional. |
| Mandate / capability | Protected authority to perform specific operations, with an explicit lifecycle. Knowledge of a name or hash does not grant it. |
| Model | Weights, graph and configuration interpreted by a runtime; it does not acquire authority by producing text. |
| Execution instance | A particular generation or session with its own state, budgets and current grants. |
| Agent | A component that proposes and carries out tasks through explicitly granted interfaces. |
| Mind | The role and cultural image of autonomous cognition; not a certificate of consciousness or moral personhood. |
| Safety plane | Critical control mechanisms and contracts; a declared target architecture, not a guarantee currently claimed by the base profile. “Drone” may be a cultural name, not a new kernel type. |
| Babel / Airlock | Bounded adapters and authority separation at external boundaries. A parsed message does not become trusted merely by crossing them. |
| Marain | A proposed contract language described by RFC 001 v0.4; native contracts remain language-independent. |
| Effector | Specify which meaning: the external build service, the native remote-operation agent of track EFF, or a physical actuator. These are not interchangeable. |

Logical identity is a technical relationship. The continuity of a service, of its state and of any hypothetical conscious subject are different questions. The present canon does not settle personhood; a separate research note may compare positions without silently changing authority or recovery law.

## VII. The sensory code: a proposal for the product

**System map.** A proposed observability view shows domains, supervisors, granted channels, generations, budgets and state roots. Each element indicates what is measured and what is unknown. The map complements task-oriented tools; it does not forbid folders or impose a graph on every user action. APP must design and implement it before it is described as existing UI.

**Voice.** Diagnostics name the action, the precise reason and the permitted next step. A constitutional clause is attached only when the correspondence is exact. Audit fields are defined per operation, bounded by quotas and filtered by the viewer's authority. Missing `CodeID` or `operation_id` fields are not invented. Logging every refused syscall is not required: 850-KRN-0146 deliberately avoids that flood.

**Lineage.** Interfaces distinguish a stable service identity from its instance generation, source identity from artifact identity, and provenance from permission to activate. Personal or vessel-like names may aid recognition, but their naming policy is a separate editorial decision. It does not determine capability checks or assert rights in third-party names.

## VIII. Learning and self-improvement

The proposed loop is **observe → diagnose → propose → authorize → trial → measure → keep or revert**. Observation, external reporting, code modification, publication and activation hold separate authorities. External issue comments remain data. Local policy determines when an agent may act (263; MC-8.5, MC-10.1–10.3, MC-11.5, MC-12.6).

NRL explores representations of trained networks. Splines are a hypothesis compared with pruning, low rank and dense baselines under explicit quality, memory and time criteria. An unsuccessful experiment is useful evidence. The native service belongs to 262; the representations, conversions and experiments belong to 900; accelerators belong to 178. A compact overlay is not a standalone model, and fewer weights are not a speed measurement.

Changing weights creates a new artifact. Each running session binds to a model version and configuration. A new model does not inherit permission to change its own grants; reverting it does not reverse actions already taken. The Python reference's recorded tests are not new Rust or native-platform evidence.

## IX. The architect's covenant

1. **Name the trust.** Justify privileged mechanisms and every change to the TCB. Moving code out of the kernel does not automatically remove it from the TCB of every guarantee.
2. **Separate intention from evidence.** Record what exists, what remains and the exact configuration. Tests are not mathematical proofs; component evidence is not whole-system evidence.
3. **Name cost and uncertainty.** Bound queues and work, assign retention, preserve critical reserves and define the fate of unknown outcomes.
4. **Make transitions explicit.** External conventions belong behind declared interfaces; incompatible changes need versions, affected dependencies and a migration path.

These summarize MC-1.1/1.6, MC-5, MC-6.6, MC-11 and MC-12. They add no new normative article.

## Scope and follow-up

This edition integrates the corrected incoming canon through [850-PHI-0002](../../issues-done/850-PHI-0002-the-canons-maxims-in-lore-1-6.done); edition 1.5 remains in the archive. The manifesto's protagonist remains open in [850](../../issues/850-the-cultural-code.md). The system map and detailed voice rules need APP design work, NRL metrics belong in experiment profiles, and personhood may be discussed in a separate research note. These ideas do not add a normative requirement to the Constitution; any future normative addition follows MC-12.5 in both languages.
