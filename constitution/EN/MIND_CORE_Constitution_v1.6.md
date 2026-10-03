# MIND CORE — Constitution v1.6

**Edition:** 1.6  
**Date:** 3 October 2026  
**Status:** current full edition; requirements for the system, not a confirmation that an implementation is ready.  
**Basis:** the full edition 1.4; amendments of edition 1.5 accepted in refined form; [review of 21 September 2026](review_2026-09-21.md).  
**Supersedes:** editions 1.4 and 1.5 (kept in [archive/](archive/)).  
**Related document:** [RFC 001: Marain, revision 0.4](RFC_001_Marain_v0.4.md).  
**Order of work:** [ROADMAP](../../ROADMAP.md).

**Edition 1.6 amendment (3 October 2026):** Articles 2–6 are restored in full from edition 1.4 (the abridgements of edition 1.5 are not accepted); the 1.5 additions to clauses 2.6 (`SHARE_RW`) and 6.10 (checkpoint protocol) are accepted in refined form; the 1.4 rule on the normative status of the appendices is restored; stable requirement IDs `MC-<article>.<clause>` are introduced; Appendix B.2 fixes the `SHARE_RW` restriction of the first profile; Appendix D links to the ROADMAP and separates the stage numberings; the categorical rationales in Appendix F are toned down. Full list in Appendix H.

**History: edition 1.4.** This edition combines the normative completeness of the Ast variant, the concrete architectural decisions of Fab and Gem, and the original Marain concept. The Ast articles are used as the basis, keeping the numbering 1.1–12.9; additions and strengthenings are listed in Appendix A. No incompatible statement is accepted silently: long-lived invariants are fixed in the articles, concrete mechanisms in the proposed engineering profile, and the limits of disputed promises in the decision map.

It is specifically the 1.3 versions that were read; historical versions 0.1–0.2 do not constitute an additional independent vote. The summary of earlier editions inside Ast is treated as part of that source. Checksums of the inputs are given in Appendix E. The source documents are not modified. The "done" and "in progress" statuses stated in them are not carried over without verifying the implementation.

**Edition 1.4 amendment (20 September 2026, integration review):** added Appendix F (rationale and rejected alternatives, consolidated from Volume II of the Fab and Gem editions and table A.3 of the Ast edition) and Appendix G (prior art and primary sources from the Fab table and Appendix D of Ast); B.1 now fixes the non-cryptographic form of local capabilities, the composition of the TCB, and monotonic clocks as a kernel primitive; B.4 — resetting or isolating a device before memory is reused; B.5 — the compiler as a bounded actor, and reference runtimes; the label of the original Marain RFC in Appendix A has been renamed so that it does not coincide with the RFC's M0–M7 stages; the file name has been corrected. Articles 1–12 are unchanged.

The document separates four levels: **Constitution → architectural specifications → platform profiles → engineering policy and roadmap**. The choice of Rust, Wasm, CSpace/CDT, Untyped, Merkle-DAG or A/B is retained as a concrete design reference point and is not presented as the only way to conform on all platforms.

## Preamble

MIND CORE is a computational substrate for an autonomous Mind and the systems it controls. Its purpose is to provide verifiable boundaries of authority, preservation of state, controlled use of resources, and continuity of critical functions under failures within an explicitly established model.

The system is designed with no obligation to reproduce the internal interfaces, process model, namespaces and permissions of existing operating systems. Existing technologies are admitted on the basis of their properties and verifiability. Compatibility is implemented through bounded adapters and does not determine the structure of the kernel.

Novelty in itself is not a criterion of quality. The criteria of MIND CORE are clarity of invariants, minimality of trust, verifiability of the implementation, and the ability to change safely.

## Scope and normative language

The words **MUST**, **MUST NOT** and **MAY** denote, respectively, an obligation, a prohibition and a permitted choice. Articles 1–12 of this edition are normative. In a conflict with a supporting RFC or engineering profile, the articles prevail; a narrower profile may strengthen requirements but not weaken them. The appendices contain explanations and implementation proposals; they do not extend the authority of components and do not override the articles. Edition 1.5 declared the engineering profile (Appendices B–E) normative; edition 1.6 restores the rule of edition 1.4: only the articles are normative. Every clause has a stable identifier of the form `MC-2.6` (Article 2, clause 6): a number is never reused in later editions, and a removed clause keeps its number marked "removed". RFCs, profiles, the roadmap and issues refer to requirements by these identifiers.

The Constitution defines the properties of the system. Architectural specifications define mechanisms. Platform profiles define hardware and constraints. The roadmap defines the sequence of work. A programming language, ISA, boot protocol, ABI format or specific algorithm does not become a constitutional requirement merely because it has been chosen for the first prototype.

**Key terms:**

| Term | Meaning |
|---|---|
| Protection domain | A boundary of authority and isolation enforced by a declared protection mechanism |
| Actor | A unit of computation with local state and a message-handling contract |
| Mandate / capability, authority descriptor | A protected reference to an object or interface that permits specific operations |
| TCB, trusted computing base | The components whose correctness is necessary for a specific declared guarantee |
| CID | An identifier of an exact immutable representation, specifying the format and hash algorithm |
| EntityID | A stable identity of a logical entity, distinct from its version and location |
| State root / Head | An authorized reference to a published version of state |
| Epoch | A generation of an object, session or authority that distinguishes a new entity from stale references |
| Execution profile | The set of supported mechanisms, guarantees and constraints of a specific configuration |
| Critical function | A function whose violation of deadlines or correctness is unacceptable under the operating model |
| Checkpoint | A recorded state with a declared consistency boundary and a recovery contract |
| External effect | An observable action beyond the boundary of a component's local state: a write, a send, a grant of a right, or an action on a device |
| Retention | An obligation to keep data for a given period or until a condition is met, taking its cost into account |
| Safety plane / cognitive plane | The critical-control circuits / the changeable cognitive subsystems |
| Logical service | A stable service entity, distinguished from a specific executing instance and its placement |

The terms "mandate" and capability are equivalent here. Capability does not mean a mandatory access-control policy based on classification labels: it is a separate model of explicit authority. The word "root" as applied to state does not mean a superuser.

---

## Article 1. Minimal trusted kernel

**1.1.** The privileged kernel MUST contain the minimal set of mechanisms necessary to enforce the boundaries of memory, authority, execution and hardware access. Admitting a mechanism into the kernel requires justifying its privileged status and assessing the change to the trusted computing base.

**1.2.** The kernel's duties include providing protected execution contexts, checking operations on kernel objects, authorized management of mappings, enforcement of execution budgets, basic cross-domain communication, and delivery of exceptions, failure notifications and hardware events. The exact set of mechanisms is specified by the platform profile.

**1.3.** Device drivers, protocol stacks, storage, interface services, application runtimes and recovery policies MUST execute outside the privileged kernel. The timer, interrupt controller, memory protection and hardware isolation mechanisms that the kernel needs are identified and accounted for separately.

**1.4.** The policy for choosing the recipients of resources is separated from the mechanism that ensures their allocation is permissible. The kernel MUST prevent unauthorized creation of mappings, objects and authorities regardless of the correctness of the user-level resource manager.

**1.5.** Access to a device includes separate accounting of registers, configuration, interrupts and DMA. Before DMA is enabled, the device-to-memory access boundary declared by the profile MUST be in place. If the hardware platform does not provide such a boundary, the device and its controlling code are included in the TCB of the corresponding guarantee; isolation from their incorrect or malicious behavior is not claimed. Such a profile cannot be used where precisely this isolation is required.

**1.6.** Unprivileged mode does not mean absence of power. A service's blast radius is determined by all the authorities, dependencies and hardware capabilities available to it. The size of the kernel is no substitute for analysis of the entire trusted computing base.

**1.7.** Creation, allocation and retyping of a protected object MUST have verifiable admissibility conditions and resource accounting. A failure, interruption or partial execution of the operation must not lead to double allocation of an exclusive resource, unaccounted objects or quota bypass. Delegating allocation policy to user space does not give it the right to bypass these conditions.

**1.8.** Reuse of memory, an execution context or a device resource MUST exclude incompatible prior access, use of stale references, and unauthorized disclosure of residual state. Scrubbing, reinitialization or another mechanism is chosen according to the resource type and the protection model. Authorized transfer of the prior contents is a separate operation and does not require their destruction.

## Article 2. Isolated state and contract-based messages

**2.1.** Each protection domain MUST have isolated state and an explicit set of authorities. Implicit cross-domain pointers and implicitly shared mutable memory MUST NOT exist. Sharing memory is permitted only through specifically granted authorities and a defined access protocol.

**2.2.** An actor and a protection domain are different concepts. Actors with a common trust boundary may be placed in the same domain. Isolation between them is claimed only to the extent provided by their runtime. For components that do not trust each other, the profile MUST specify a mechanism that preserves their boundary under the adopted attack model.

**2.3.** Native cross-domain communication MUST use interfaces with explicit types and versions. The contract defines the format and limits of the data, the authorities permitted to be transferred, errors, message ordering, termination, cancellation, overflow, and the consequences of failure of the parties.

**2.4.** The kernel checks authorities, descriptor admissibility, envelope boundaries and resource limits. Application schemas and the meaning of messages are checked in isolated adapters on the receiver side. A message MUST NOT be considered trusted merely on the basis of its type identifier or the sender's promise to use a safe language.

**2.5.** Byte arrays and streams are permitted as explicit types. A stream may be long-lived or unbounded in the number of elements, but its buffers and resource consumption MUST be bounded. The contract MUST specify backpressure, termination and cancellation. Queues with unbounded accumulation MUST NOT be used.

**2.6.** Memory transfer MUST distinguish copying of data (`COPY`), transfer of exclusive ownership (`MOVE`), shared immutable reading (`SHARE_RO`) and bounded temporary granting of access (`LEASE`). Shared immutable reading requires that no writes to the given version of the contents be permitted for the entire duration of the reading, including writes through other mappings and DMA. A single read-only mapping on the receiver side is not sufficient. Shared mutable access (`SHARE_RW`) requires a separate contract for synchronization, coherence and responsibility. In secure profiles `SHARE_RW` is restricted to the components and modes listed in the profile (Appendix B.2). Trust in an adapter that receives such access does not prevent the other side from changing the buffer: the adapter validates a bounded private copy, enforces immutability of the buffer while it is in use, or applies a separately justified concurrent algorithm (2.11).

**2.7.** For an exclusive memory transfer, a successful operation MUST exclude further access by the previous owners, and access via derived rights they issued, to the entire transferred object within the declared protection model. This includes CPU and DMA mappings and the ability to recreate them. If this condition cannot be met, the operation is rejected or another explicitly agreed mode is used. Contents outside the transferred range must not be disclosed to the recipient. This terminates access under the previous authorities and does not prohibit a separate subsequent authorized transfer or grant of rights compatible with the current ownership mode.

**2.8.** The transfer contract MUST define the point at which ownership passes, the state of the buffer in the queue, and the outcomes of cancellation and death of the parties. Between the moments of transfer there must never be two owners of exclusive mutable access. A failure must not leave a resource unaccounted for or without a release path.

**2.9.** Zero-copy is permitted as an optimization provided the semantics are preserved. The absence of copying MUST NOT be taken to imply zero cost, constant latency or independence from the number of pages and devices. The actor model in itself is not proof of the absence of logical races, cyclic waits or starvation.

**2.10.** For protocols on which critical guarantees depend, the states of the parties, admissible transitions, wait dependencies and progress conditions MUST be defined. Verification takes into account bounded queues, budgets, cancellation, repeated messages, failure of a party, and interaction with other protocols. A guarantee of freedom from deadlock or starvation is stated together with the model and the assumptions about scheduling, communication and load. Type checking or checking of a single channel is not considered proof of progress of the whole composition.

**2.11.** Validation of a message MUST remain in force until the validated values are used. For memory that an untrusted party can modify concurrently, the contract specifies a way of obtaining a stable representation or an algorithm for safe access under such modifications. A one-time validation of a mutable buffer does not permit re-reading arbitrarily changed lengths, addresses and descriptors from it as validated. Parsing, decompression and traversal of nested structures are subject to their own limits and work accounting.

**2.12.** Receiving capabilities is part of the message-acceptance contract. The admissible kinds, number, transfer mode and acceptance conditions of authorities MUST be defined, as well as the fate of descriptors that are not accepted. An unexpected authority does not automatically extend the recipient's role and does not oblige it to accept unbounded storage or processing costs.

**2.13.** Temporary granting of access (`LEASE`) MUST define the permitted operations, the clock, the term, renewal, and the moment access actually ends. Expiry of time does not in itself prove that CPU/DMA access has ended: reuse of the resource is permitted only after the established termination and cleanup procedure. Revocation and expiry of a lease do not undo actions already performed.

## Article 3. Explicit authorities and their lifecycle

**3.1.** Access to a protected object MUST rest on an explicitly granted capability. A name, path, hash, type, process identifier or membership of a user do not in themselves grant authority.

**3.2.** A local capability MUST be protected against forgery and amplification by a trusted mechanism. An application may hold a numeric handle, but manufacturing or guessing a number does not create an authority. The concrete implementation — a protected table, a hardware mechanism or another verified option — is specified by the architectural specification.

**3.3.** Creating a new domain does not give it implicit access to the creator's authorities. Initial resources and channels are passed explicitly. Delegation does not depend on process kinship; the permitted recipients and conditions are defined by the authority's contract.

**3.4.** Operations for transferring, copying and attenuating a capability MUST be distinct. A derived authority cannot exceed the original in permitted operations and scope. Obtaining new power from another authorized subject is a separate grant event, not an amplification of an existing capability.

**3.5.** For each kind of authority, the permitted operations, delegation mechanism, copyability, lifetime, revocation rules and the fate of operations in progress MUST be defined. Complex domain-specific restrictions may be enforced by narrow intermediaries; the kernel is not obliged to understand their meaning.

**3.6.** Revocation MUST have a declared scope and observable completion. After it completes, new operations through the references covered by the revocation are not permitted. Reuse of internal identifiers must not revive stale references. Revocation does not undo actions already performed and does not erase information previously obtained.

**3.7.** A universal bypass of authority checks on the basis of identity, process name or a special user number MUST NOT exist. Initial and recovery authorities MUST be explicitly described, separated by purpose and accounted for in the TCB. After boot, unused authorities are removed; broad authorities that are retained require justification and a restricted scope.

**3.8.** A service acting on behalf of a client MUST bind the request to the permitted scope of actions. Use of its own broader authorities is permitted only as an explicitly specified function of the service. This excludes the implicit conversion of any client request into a privileged action.

**3.9.** Compromise analysis MUST take into account transitively reachable capabilities: available services, further granting of rights and cooperation between components. A prohibition on direct transfer of a capability is not considered a prohibition on proxying its use without a separate information-flow control model.

**3.10.** Authentication, granting of rights, checking of an operation and auditing MUST be distinguishable. A capability permits an action but is not obliged to attest to the identity of its current holder. Cryptographic serialization of remote authorities is also governed by Article 7.

**3.11.** A manifest, an effect declaration and saved state describe a component's needs but are not in themselves an authorization. The granting of authorities at launch, recovery or binding MUST rest on a valid authorization. Additional power MUST NOT be obtainable merely by replacing the declaration of required rights.

**3.12.** Boot MUST have a verifiable boundary at which the initial distribution of authorities is complete. Before untrusted components are admitted, unused boot authority is removed, and the retained recovery authorities are explicitly isolated, restricted and accounted for in the TCB. The bootloader's relinquishing of rights is not considered to have taken place while an unaccounted path for regaining them remains.

**3.13.** Granting, delegation and replenishment of a resource budget MUST be authorized and subject to accounting. Derived reserves do not create additional resource beyond the allocated amount. A copy of a descriptor does not duplicate the budget; shared consumption and transfer of work to an intermediary have an explicit payer. The concrete form of the budget capability is specified by the profile.

## Article 4. Immutable data and explicit state

**4.1.** Persistent storage MUST separate the identity of an immutable representation from the logical name, access rights and physical placement. The native model does not require a single directory tree. Trees, collections and tags are permitted as views over objects.

**4.2.** Published objects and structural manifests MUST be immutable and verifiable by content. The identifier format provides for a version and identification of the hash algorithm. The rules for encoding and for binding the type to the hashed data MUST be unambiguous. For opaque data, the exact byte sequence is identified, with no assumption that different representations have the same meaning.

**4.3.** A stable logical entity, its immutable version and the reference to the current version MUST be distinct. A change creates a new version and performs an authorized publication of a new root. Concurrent updates are resolved by a declared protocol; confirmed changes MUST NOT be silently lost outside the agreed semantics.

**4.4.** Publication of a root MUST take place after the declared level of durability has been reached for all data needed to recover that state. References to external or not-yet-received objects are marked separately and are not presented as stored data. The protocol defines the outcome of a partial write, a power loss and an incomplete transaction.

**4.5.** Version history MUST have explicit links and a retention policy. The garbage collector takes into account published roots, pinned snapshots, operations in progress and obligations to consumers. For references, retaining an object is distinguished from merely mentioning it. A temporary loss of connectivity with a node does not in itself prove that the roots it stores have ceased to exist.

**4.6.** Semantic links MUST contain a type, provenance and an interpretation version. A model's inference, an embedding and a tag are assertions by a specific source, not an immutable truth about the data. Indexes and search views must have a verification and recovery path; search results do not bypass access checks on objects and metadata.

**4.7.** Integrity, provenance, confidentiality, availability and durability MUST be considered separately. A hash verifies correspondence to a representation; it does not permit reading and does not prove currency. Deduplication across different confidentiality domains is permitted only after an explicit assessment of the leakage of the fact that the data coincide.

**4.8.** The storage policy MUST define corruption detection, recovery, independence of copies and media exhaustion. Deleting data, deleting references and destroying keys are different operations. Their guarantees do not extend to uncontrolled copies already handed over.

**4.9.** Immutability of persistent objects does not prohibit local mutable working state, caches and temporary buffers. Writing every intermediate computation to persistent storage is not required. The commit points and the permissible loss of temporary state are specified by the component's contract.

**4.10.** Publication of a change to several roots MUST declare the boundary of atomicity, isolation and durability. If the contract requires consistent observation of these roots, it specifies the corresponding read method; within this boundary, prohibited intermediate states are not observed. Outside the boundary, the partial result, conflict and recovery order are defined explicitly. The existence of CAS does not mean that a universal transaction exists across all nodes of the system.

**4.11.** The right of access to an object and the obligation to retain the object MUST be distinct. Retention has a party responsible for accounting, a term or termination condition, and a defined link to a quota. A capability may grant both access and retention only under an explicit contract. Loss of a temporary handle does not cancel storage obligations, and copying a read right does not create unbounded unaccounted retention.

**4.12.** Content identity, semantic assertions, placement, replication state and valid authorities MUST be distinct in the data model. A derived search index is not in itself a source of authority, proof that a version is current, or permission to delete the source data. For each kind of metadata a source of truth is defined; necessary control state is not declared recoverable from content without a corresponding protocol.

**4.13.** A change of hash algorithm or encoding MUST have a migration protocol for objects, references, roots and retention. Correspondence between the old and new identifiers is verified by the declared content transformation; an identifier alias does not grant new rights. An unknown format or a prohibited algorithm is not silently accepted as supported.

## Article 5. Time and resources

**5.1.** Processor time, memory, kernel objects, capabilities, queues, disk space, I/O and interrupt-handling work MUST have an accounting owner, a limit or an explicitly reserved budget. Work done by the kernel and services on behalf of a client is also subject to accounting.

**5.2.** Admission of load MUST take available resources into account. Budget exhaustion leads to a defined outcome: refusal, bounded waiting, degradation of quality or agreed preemption. Overload MUST NOT be compensated for by unbounded accumulation of requests.

**5.3.** For critical functions, deadlines, permissible jitter, maximum pauses and the response to their violation MUST be specified. Guarantees are derived for a specific platform and load, taking into account shared memory, caches, buses, interrupts and accelerators. Average latency is not considered a deadline guarantee.

**5.4.** Non-preemptible sections capable of delaying a critical function MUST have a justified upper bound. Long-running revocation, cleanup, scanning and recovery operations must be bounded, splittable or preemptible according to the profile. Interaction between services must take into account priority inversion and the transfer or reservation of budget.

**5.5.** Reserves for critical functions and their recovery MUST be preserved under overload of non-critical components. Background indexing, training, compilation, garbage collection and diagnostic traffic cannot implicitly consume these reserves.

**5.6.** Durations, deadlines and the order of events MUST rest on explicitly defined clocks. A correction of calendar time must not silently change the deadline of a local operation. Inter-node guarantees that depend on time require an explicit tolerance for clock skew or a protocol that does not rely on such synchronization.

**5.7.** If a profile supports modes of differing criticality, it MUST define the conditions for transitions between them, the fate of preempted work, the reserves preserved and the conditions for returning. Degrading the quality of non-critical services does not permit a hidden transfer of their obligations and costs onto critical circuits.

## Article 6. Failure containment and recovery

**6.1.** For a component, the failure boundary, dependent functions, source of observation and permissible recovery outcomes MUST be defined. A failure does not reduce to process termination: hangs, deadline violations, incorrect results, resource exhaustion and state corruption are taken into account.

**6.2.** The kernel provides containment of those violations it is able to detect and delivers a notification to the designated handler. The choice of restart, failover to a standby or degradation is made by a separate policy with the minimum necessary authorities.

**6.3.** Recovery MUST reconcile the state of the process, its authorities, queues, devices and published data. Before DMA memory is reused, the necessary operations to stop access, invalidate translations and clean up must be completed. The death of a driver alone does not confirm that DMA has stopped.

**6.4.** A new instance of a component MUST be distinguished from the old one by a generation or an equivalent verifiable mechanism. Messages to the old instance, unfinished replies and saved handles are processed under an explicit protocol. Applying them automatically to the new state without verification is prohibited.

**6.5.** Repeated restarts MUST have a budget. Exhaustion of the budget leads to defined degradation, quarantine or failover. A supervisor must not depend on a resource that the component being recovered is able to exhaust completely.

**6.6.** Acknowledgment of message delivery does not mean completion of the action. For repeatable requests, operation identifiers, idempotency and deduplication rules are defined. An exactly-once effect guarantee is permitted only where there is a protocol covering both the state and the effect itself. A timeout does not prove that the action did not take place.

**6.7.** Automatic detection of any compromise MUST NOT be claimed. Silent production of false results requires additional checks. Fault-freedom of the kernel does not mean continuity of service, and a restart does not undo physical actions already performed.

**6.8.** Before a component is launched, the party responsible for its lifecycle, the recipient of failure notifications and the escalation path MUST be assigned. For failure of the supervisor itself and of the initial controlling components, a final recovery boundary or degradation mode is defined. Boot and recovery dependencies, including access to images, storage and keys, must not form an unresolvable cycle within the declared failure model.

**6.9.** Spawned work MUST have an owner of its lifecycle and resource accounting. Completion or cancellation of a work scope determines the fate of descendants and unfinished operations. Continuing work outside this scope is permitted after an explicit transfer of responsibility. Cancelling the wait for a reply does not mean cancelling an external action that has already been accepted.

**6.10.** For a component that claims to save and restore state, a versioned checkpoint contract MUST exist: the composition of the state, the consistency boundary, dependencies, the permissible loss of changes, and the procedure for verifying recovery. The contract reconciles the saved state with accepted messages, unfinished replies and external effects, including effects with an indeterminate outcome. The necessary code, schemas and data are retained for the period of promised recoverability. Persistence is not automatically assumed for every actor. The persistent-execution profile MUST define a consistent checkpoint protocol for state, queues and unresolved effects: where the consistency point is fixed, who keeps receipts, when the sender may be acknowledged, and what happens on a failure between an effect and the recording of its result. The responsible components, including the kernel and the runtime, provide the bounded operations this needs without violating isolation boundaries; the mechanism (quiescence, a log, barriers or another) is described in the profile. Domain logic of checkpoints is not placed in the kernel.

**6.11.** A checkpoint MUST distinguish data, references to resources and authorities. Memory addresses, local handles and device state are not considered a portable access permission. Restoration, cloning and migration do not automatically return revoked rights and do not duplicate exclusive power. Before protected actions resume, current authorization, generations and rebinding conditions are checked; if they are not met, the declared refusal or degradation mode applies. Restoration of the state of the authorization system itself MUST also preserve the adopted guarantees of revocation and protection against impermissible rollback.

**6.12.** The logical identity of a service, the generation of its instance and the place of execution MUST be distinct. Reassignment of a logical endpoint is an authorized operation with a protocol for handling in-flight messages. Migration and handover of exclusive control MUST prevent simultaneous acceptance of new commands from the old and new owners. Whether replaying old requests is permissible is determined by the operation's contract, not by the service name matching.

## Article 7. Distribution and partial connectivity

**7.1.** Local and remote interaction may use common message schemas but MUST differ in their guarantees of latency, delivery, availability and failure. A remote call must not hide the possibility of an indeterminate outcome.

**7.2.** For distributed state, the tolerated failures, consistency model, write conditions and behavior under network partition MUST be declared. Simultaneously promising unconditional write availability and a single immediately consistent version under arbitrary network partition MUST NOT occur.

**7.3.** Replication takes into account common causes of failure: power, controllers, physical placement, software version and keys. Several instances on one shared resource are not considered independent merely by their number.

**7.4.** After a handover of control, a resource MUST reject new commands from a stale controlling node. Epochs, fencing or another mechanism are checked at the boundary that actually accepts the resource's commands. The fate of previously accepted commands is determined by the handover protocol.

**7.5.** A remote authority MUST have protected provenance, a scope of application and rules for re-presentation. The protocol MUST define protection against impermissible replay of requests and re-execution of actions. For an authority, a term, an audience and a revocation method are specified. Instant revocation of an authority that is verified autonomously on an unreachable node is not assumed.

**7.6.** Loss of connectivity must not arbitrarily destroy necessary local rights of critical control. The policy for leases, autonomous mode and subsequent reconciliation is specified before operation. The choice of safe behavior on loss of connectivity is part of the contract of the specific function.

**7.7.** Placement and migration MUST take into account topology, communication cost, device availability and shared causes of failure. A common interface for local, inter-core and inter-node interaction does not eliminate these differences. Changing the placement of a critical function requires confirming its timing, resource and isolation prerequisites for the new configuration.

## Article 8. Cognitive execution and physical control

**8.1.** The Mind and its cognitive subsystems use the same authority mechanisms as all other components. Intelligence, trusted origin or a program's self-description do not grant a bypass of system boundaries.

**8.2.** Changeable models, planners and exploratory computation (cognitive plane) MUST be separated from the mechanisms executing critical physical actions (safety plane). Cognitive components pass proposals through a bounded contract to control components and do not receive direct authorities over critical actuators. The control component checks the authority, the admissible state, the parameters and the sequence of the action at the execution boundary. The fact that a proposal is typed does not prove its physical admissibility.

**8.3.** A physical-control contract MUST define the scope of action, parameter limits, deadlines, admissible sequences and transitions on failure. The substantive decision may be made by the Mind; the right to change the limits themselves is separated from the right to execute ordinary commands.

**8.4.** Local critical circuits MUST have resources and a mode of continued operation or controlled degradation upon failure of the cognitive subsystem. There is no universal rule of "on any error, switch everything off": the admissible mode is determined by the physics of the controlled process.

**8.5.** Self-modification of code, models and policies is permitted through verifiable update procedures. A change of knowledge, a change of executable code and a change of authorities are distinguished. None of these operations must implicitly perform the others.

## Article 9. Boot, update and provenance

**9.1.** The platform profile MUST describe the initial root of trust, the boot sequence and the provenance of executable code. At boot, the authorization of the selected image and configuration is verified; all earlier unverifiable assumptions are stated explicitly.

**9.2.** An executable image MUST be bound to a dependency manifest, interface schemas, required authorities and execution conditions. A signature establishes authorized provenance within the given trust model; it does not prove the absence of errors.

**9.3.** An update MUST have a defined activation point, handling of unfinished operations and state migration. A failure at any point must leave a recoverable configuration within the declared failure model. Recovery images and the objects they need are protected from ordinary storage cleanup.

**9.4.** Unauthorized rollback to a vulnerable configuration MUST NOT be possible. A permitted rollback must take into account state compatibility, the minimum permissible version policy and separate recovery authorities. Expiry of metadata during autonomous operation is handled according to a predefined policy.

**9.5.** Keys, algorithms, schemas and formats MUST allow replacement without implicitly removing verification. Build reproducibility and toolchain provenance are included in the trust evidence to the extent required by the profile.

**9.6.** The key lifecycle MUST define creation, assignment, storage, use, rotation, revocation and destruction. The compromise protocol defines the fate of previously authorized images, authorities and data, the availability of recovery and the actions of autonomous nodes. Keys for different purposes and recovery power are separated according to the threat model. Replacing a key does not undo the consequences of previous data disclosure or of actions already performed.

**9.7.** The provenance of a derived executable artifact MUST bind it to unambiguous inputs, dependencies, tools and build parameters. For a claimed reproducible build, the environmental conditions affecting the result are recorded; a symbolic dependency name cannot silently resolve to different content. A matching source-code identifier does not prove matching behavior in different environments.

**9.8.** Reuse of a compiled artifact MUST verify its integrity, provenance and suitability for the current execution. The lookup key of a derived artifact and the verified metadata together cover the input modules and dependencies, the toolchain, settings, target platform, hardware feature set, ABI/runtime and the applicable security profile. A hash of the Wasm module alone is not sufficient. CAS attests to the identity of the stored bytes, but not to the correctness of compilation; trust in the compiler or verification of the result is included in the justification of the corresponding guarantee.

**9.9.** An update of a running service MUST define the permissible interruption, the readiness criterion for the new version, the fate of sessions, state compatibility and the control handover protocol. For a critical function, adherence to its contract throughout the transition, or a pre-authorized maintenance mode, is confirmed. Coexistence of versions requires resources and authorities for both and does not permit dual exclusive control. A rollback of the program is not considered an undoing of external effects. If the continuity conditions are not met, the update is not declared seamless.

## Article 10. Information flows and observability

**10.1.** The right to read data and the right to pass it to another domain are different aspects of policy. If restriction of dissemination is required, the profile MUST specify an information-flow mechanism and its assumptions. The capability model in itself is not considered a guarantee of non-disclosure after reading.

**10.2.** Observation, logs, tracing and debugging MUST be subject to authorities and quotas. Diagnostic interfaces cannot create an undeclared path for reading memory or bypassing isolation.

**10.3.** For critical decisions and changes of authorities, a minimal audit trail MUST be defined: the object of the action, the authorization context, the generation and the result. Logs must not retain live bearer tokens or secrets without separate justification and protection.

**10.4.** For critical circuits, sufficient evidence for failure analysis MUST be defined: versions of code and state, external inputs and significant non-deterministic events. A complete record of all computation is not required; its limits and cost are specified explicitly.

**10.5.** Protection against side channels MUST be declared separately from ordinary memory isolation. The profile defines the protected channels, measures and residual risks. Leakage MUST NOT be declared zero merely on the basis of separate address spaces or the use of Wasm.

**10.6.** The audit contract MUST define the provenance and integrity of records, detection of gaps within the model, the retention period and the behavior when recording is impossible. A component cannot alter without control the evidence on which a declared independent verification of its actions is based. Audit failure must not implicitly halt a critical function or permit an action that requires prior recording: the corresponding mode is specified before operation.

**10.7.** If the profile restricts dissemination of data, permitted downgrading of classification or release of data (declassification) MUST be a separate authorized operation with a defined recipient, scope, basis and audit. An ordinary right to read, forward a message or write a log entry is no substitute for such authorization.

## Article 11. Languages, runtimes and the Babel boundary

**11.1.** Native MIND CORE contracts MUST be independent of the internal object representation of any particular language. ABIs and schemas are described explicitly. Unsafe code, foreign interfaces, compilers and runtimes are accounted for in the guarantees of the domain that trusts them.

**11.2.** A safe language or sandbox does not exempt from checking authorities, input data and the resource budget. Guarantees within a language and guarantees between domains MUST be distinct. Changing the language must not change the meaning of a granted capability.

**11.3.** External protocols, file formats and compatibility environments MUST be connected through isolated Babel adapters. Their internal conventions about names, users, processes and errors do not define the native semantics of the kernel.

**11.4.** An adapter MUST receive minimal authorities for a specific session, medium or operation. Parsing of external data, use of secret keys and execution of a privileged effect are separated to the extent required by the threat model. Each additional boundary is assessed by its reduction of power, its cost and its recovery complexity.

**11.5.** Converting external input into a typed message does not make it trustworthy. The provenance, constraints and uncertainty of the data are preserved. Access policy and domain invariants are also checked on the side of the executing service.

**11.6.** Babel's access scope MUST restrict the permitted network destinations, objects and actions according to the adapter's purpose. Its restart is subject to Article 6. A parser does not receive direct control of a device merely because it serves that device's protocol.

**11.7.** A component MUST have an explicit description of the required external interfaces, authorities and effect classes within the adopted execution model. Whether this description can be checked statically is stated separately. Checking effects at compile or link time does not replace a valid authority at run time and does not cancel subsequent revocation. Binding an effect handler is itself subject to the rules for granting authorities.

**11.8.** Every step outside the guarantees of a language or runtime MUST have explicit assumptions, caller obligations and invariants provided. Such boundaries are localized and included in the verification of the implementation. This applies to unsafe code, FFI, hardware access, native extensions and automatically generated transitions between environments.

**11.9.** Access to secret key material and the right to request a cryptographic operation MUST be distinct. If the profile separates the parser and the key service, the parser receives only the necessary rights to operations with a given context, purpose and budget. The impossibility of extracting a key does not mean the impossibility of abusing a permitted signature or decryption. Access to the plaintext of different sessions is restricted separately.

**11.10.** A projection of an external medium or remote object MUST preserve provenance, availability conditions, and the version or a mutability indicator. A change of the source, its disappearance and a session break have a defined outcome. An external projection is not declared a durably stored immutable object until an authorized import, with verification of the content being committed and fulfillment of the durability contract. The right to write to the external source is separated from reading and importing.

**11.11.** A domain parsing untrusted external input MUST NOT be combined with universal granting of authorities, broad storage access, extraction of long-term keys or direct control of critical devices. For an adapter, an authority map of the individual stages MUST exist: transport, cryptographic operations, parsing, policy and application effect. The number of processes is not fixed; merging stages is permitted only if these restrictions and the declared failure boundary are preserved.

## Article 12. Verifiability, conformance and amendment of the Constitution

**12.1.** Each declared guarantee MUST have a scope: implementation version, configuration, platform, failure and threat model, trusted computing base and assumptions. The proof for one component MUST NOT be carried over to the whole OS without a corresponding compositional justification.

**12.2.** For critical mechanisms, an invariant specification and a plan for obtaining evidence MUST exist: a formal model, machine-checked proofs where they are claimed, implementation analysis and testing of the corresponding failures. Test results are not called a mathematical proof.

**12.3.** A conformance profile MUST list implemented and unimplemented requirements. A prototype may be incomplete but must not present design intentions as existing guarantees. An unremedied violation of a mandatory article means absence of full conformance to this edition.

**12.4.** An approved interface has a version, evolution rules and a migration procedure. A new version may change semantics only explicitly. Support of an old version is permitted in a bounded adapter with a defined lifecycle; silently preserving incompatible assumptions in the kernel is prohibited.

**12.5.** An amendment of the Constitution is issued as a separate edition with the reason, the guarantees affected, alternatives, a migration plan and the consequences for verification. An implementation cannot retroactively redefine a requirement in order to claim conformance.

**12.6.** The right to change system policy or to activate a new execution edition MUST be separated from ordinary application authorities. The mechanism for authorizing these changes is specified by the system's governance model; the Constitution does not require a universal human administrator and does not grant the Mind an unbounded bypass of control.

**12.7.** Interface compatibility MUST be assessed in terms of data representation, protocol behavior, authorities, errors and resource-and-timing conditions. A new field or message is not automatically considered compatible. An incompatible change requires a distinguishable contract and an explicit transition: coexistence of versions, adaptation or an agreed shutdown of dependent components.

**12.8.** For an interface being retired, the end-of-support conditions, detection of remaining dependencies and a migration path MUST be defined. Extending support is an explicit decision with responsibility and cost. Expiry of a calendar deadline does not in itself permit violating the contract of a critical function.

**12.9.** A change of code, configuration, node membership, placement or resource policy MUST determine the guarantees affected. Before activation, their necessary prerequisites for the new configuration are confirmed. Proofs and verification results that have become inapplicable are not automatically carried over to it.


---

# Appendix A. Integration of the three variants and resolution of discrepancies

Notation: **Ast** — `MIND_CORE_Constitution_v1.3ast.md`; **Fab** — `MIND-CORE-Конституция-v1.3fab.md`; **Gem** — `const_gem_1.3.md`; **Mar0** — the original Marain RFC `const_gem_1.3.marain.md`. This is a map of content, not an assessment of the authors. "v1.4 refinement" denotes an editorial synthesis, not a verbatim norm from a source.

| Topic and contributions of sources | v1.4 decision | Where fixed |
|---|---|---|
| Minimal TCB: all; memory mechanisms: Fab 1.2, Gem 1.2 | Policy outside the kernel; protected object lifecycle is mandatory; Untyped/retype is the profile's baseline mechanism | 1.1–1.8; B.1 |
| IOMMU and a device without one: Fab 1.3, Gem 1.3, Ast 1.5 | An untrusted DMA device is not admitted to a profile with an isolation guarantee without a hardware boundary; any other profile explicitly includes it in the TCB | 1.5; B.1 |
| Actor versus domain: Fab 2.1, Ast 2.2 | One trust zone may combine actors; a separate boundary for mutually untrusting components | 2.1–2.2 |
| Channel contracts: Fab 2.2/2.7, Gem 2.1, Ast 2.10 | IDL specifies representation; the state machine and composition model specify ordering and progress; WIT by itself does not prove them | 2.3–2.5, 2.10; RFC §§7, 11 |
| COPY/MOVE/SHARE_RO/LEASE/SHARE_RW: all | All modes, the transfer point and CPU/DMA conditions retained; an explicit LEASE termination contract added | 2.6–2.9, 2.13 |
| Untrusted shared buffers and incoming capabilities: Ast 2.11–2.12 | Stable-representation checking and bounded acceptance of descriptors retained | 2.11–2.12 |
| Local caps: Fab 3.2, Gem 3.1; hardware neutrality: Ast 3.2 | In the first profile, CSpace and handles not transferable as bytes; in the Constitution, unforgeability and checking; CHERI remains a porting direction | 3.1–3.2; B.1 |
| Delegation, CDT and revocation: Fab 3.3–3.4, Ast 3.4–3.6 | Observable completion is mandatory; CDT chosen for the local profile; independently granted rights and remote revocation are distinguished | 3.4–3.6, 7.5; B.1 |
| Boot authority: all | Unused power removed before untrusted code; bounded isolated recovery power retained explicitly | 3.7, 3.12 |
| Confused deputy and proxying: Fab 3.6, Gem Vol. II, Ast 3.8–3.10 | Capabilities help restrict action, but a service is obliged to bind a request to the permitted scope; transitive power taken into account | 3.8–3.10 |
| Budget as a capability: Fab 3.7/5.1, Gem 5.1; accounting for intermediaries: Ast 5.1 | Budget-rights mechanism retained; the v1.4 refinement prohibits multiplying a reserve by copying a handle | 3.13, 5.1–5.7 |
| CID/EntityID/Head: all; Merkle/chunking: Fab 4.1–4.2, Gem 4.1–4.2 | Three identities mandatory; Merkle-DAG and choice of chunking belong to the profile; universal O(log n) is not promised | 4.1–4.4; B.3 |
| Atomicity of several Heads: Fab 4.3; limits: Ast 4.10 | A declared transactional boundary and consistent reads are mandatory; a global transaction is not assumed | 4.10 |
| GC by capabilities: Fab 4.5, Gem 4.3; retention: Ast 4.11 | Roots are registered and paid for; reading, mentioning, retention and history are distinguished | 4.5, 4.8, 4.11 |
| Assertions, indexes, hash agility: Fab 4.4/4.6, Ast 4.6/4.12 | Provenance of an assertion preserved; an index grants no rights; the v1.4 refinement specifies hash migration | 4.2, 4.6–4.7, 4.12–4.13 |
| Mixed-criticality: Fab 5.2, Gem 5.2, Ast 5.7 | Enforced reserves and modes retained; timing proofs take shared hardware into account | 5.3–5.7; B.4 |
| Supervision tree: Fab 6.1, Gem 6.1; full responsibility: Ast 6.8–6.9 | Every actor has a lifecycle owner; the tree is the baseline profile; recovery of the supervisor is not left out of scope | 6.1–6.9; B.4 |
| Restart sequence: Fab 6.2, Gem 6.2; unknown effect: Ast 6.6 | The budget is checked before a new launch; revocation, fencing and DMA termination precede dangerous reuse; undoability is not invented | 6.3–6.6; B.4 |
| Orthogonal persistence: Fab 6.5, Gem 4.3; precise contract: Ast 6.10–6.12 | A persistent-actor profile is supported; a snapshot does not restore rights automatically and is not mandatory for every actor | 6.10–6.12; RFC §9 |
| Multikernel: Fab 7.1; topology: Ast 7.7 | Retained as an architecture candidate; local and remote calls are distinguished; the SMP/multikernel choice requires evaluation | 7.1–7.7; B.1 |
| Remote caps, lease, fencing: all | Cryptographic profile and epoch checking at the resource; terms are reconciled with autonomous control | 7.4–7.6; B.4 |
| Safety/cognitive: Fab 8, Gem 5.2, Ast 8 | 8.2 strengthened: cognitive code proposes an action through a control component without direct power over a critical device | 8.1–8.5 |
| A/B, rollback, keys: Fab 9, Gem 8.1; provenance/AOT: Ast 9 | A/B is the initial profile; full build recipe, migration, key lifecycle and safe control handover retained | 9.1–9.9; B.5 |
| Hot update: Fab 12.1; continuity conditions: Ast 9.9 | The goal is retained with an interruption budget and state; permissible maintenance is stated explicitly | 9.3–9.4, 9.9, 12.7–12.9 |
| Information flows: all; audit: Ast 10.6 | A separate dissemination mechanism; the v1.4 refinement singles out authorization of declassification | 10.1–10.7 |
| SMT/scrubbing: Fab 10.3, Gem 8.3; limits: Ast 10.5 | The prohibition on shared SMT for protected critical domains is included in the baseline profile; the list of channels and residual risks are explicit | 10.5; B.1 |
| Babel: Fab 11, Gem Babel section; key handles and projections: Ast 11.9–11.10 | Network and block paths retained; the prohibition on combining broad rights with raw input strengthened; a fixed number of processes is not required | 11.3–11.11; B.6 |
| Rust/Wasm/Marain: Fab Vol. III, Gem languages, Ast 11 and Appendix B, Mar0 | Rust is the initial systems language; Wasm is the target format; Marain is a separate RFC with verifiable stages and guarantee boundaries | 11.1–11.2, 11.7–11.8; B.5; RFC |
| Conformance and migration: Fab 12, Ast 12; early assurance: Fab/Gem roadmap | Explicit profile, applicability of proofs and migration retained; unconfirmed statuses excluded | 12.1–12.9; C, D |

**Not carried over literally:** "all races are excluded by actors", "zero-copy is free", "an AST hash eliminates incompatibility", "an effect equals a permission", "Perceus proves hard RT", "a crash means a detected compromise", "a restart undoes an external action". In every case, the useful mechanism is retained and the missing contract is added. Identity and ACLs may be used in the rights-granting service or in Babel; they do not create a universal bypass of capabilities. The full list of rejected theses with reasons is in Appendix F.

# Appendix B. Baseline engineering profile

This is an agreed proposal for the first implementations, not evidence that the listed mechanisms exist. An alternative mechanism is permitted provided the articles are preserved and the profile is explicitly revised.

## B.1. Kernel, platforms and authorities

First profile: x86-64, UEFI, SMP and hardware DMA isolation; the verification testbed is QEMU/VirtIO, followed by physical devices. The hypervisor, firmware, bootloader, IOMMU configuration, toolchain, runtime and the hardware itself are accounted for in the TCB of those guarantees that depend on them. IOMMU support is proven by configuration and testing, not by the name of the platform.

Initial object model: Untyped pools and verifiable retype; CSpace with local handles and generations; endpoints; memory objects; device/IRQ objects; scheduling contexts and budget rights. For derived local capabilities — a CDT or equivalent provenance tracking. Complex restrictions on destinations, ranges, rate and number of operations are implemented by intermediaries with minimal power. Copying a cap and independently granting a right are distinguished on revocation. Local capabilities are not represented as cryptographic tokens and are not transferred as bytes: an application operates on a handle in a protected table, and manufacturing, copying outside the trusted mechanism, or guessing the number does not create a right (3.2, 3.6). The cryptographic form is applied only to remote authorities via a gateway (7.5). Monotonic clocks are a kernel primitive on which budgets, deadlines and the scheduler rely; calendar time is a separate service with its own authority, and its correction does not affect local deadlines (5.6).

Rust is the baseline systems language. Unsafe/FFI/MMIO are concentrated in narrow modules with contracts and separate verification; application services use safe bindings. Affine ownership helps avoid using a moved wrapper, while the actual protection of the resource is provided by the trusted mechanism. The toolchain and dependencies are pinned.

Multikernel is retained as a candidate for NUMA and scaling; the decision between a shared SMP kernel, a multikernel and a mixed variant is made on the basis of the coherence, failure, IPC and cost models. RISC-V and CHERI/Morello are porting directions requiring new ABIs, hardware assumptions and evidence.

In the profile protecting critical domains from microarchitectural leakage, shared SMT with untrusted workloads is prohibited; available state partitioning/flushing mechanisms are applied for explicitly listed channels. If the required protection is unattainable on the platform, the profile is not claimed. This is not a promise to eliminate all side channels.

## B.2. IPC and MIND IDL

Small messages are transferred by copying; large ones through agreed memory objects. Ring buffers are permitted for sustained streams with a separate shared-memory contract, quotas and robust data validation. The threshold between COPY and other modes is determined by measurement and the timing profile. Enqueue and transfer of exclusive ownership have a single commit point; the sender knows whether it still holds the resource on failure.

MIND IDL includes a data schema, a version, size limits, kinds of capabilities and a separate behavioral contract. WIT/Component Model is the baseline candidate for representing interfaces; finite-state machines/session types supplement it. For critical paths, channel composition, overflow, priorities and counterparty failure are also verified. A fast path does not exclude verification of the slow path, object deletion, errors and revocation.

**`SHARE_RW` in the first profile.** The secure Core profile, including Marain Core MVP, does not use `SHARE_RW`. It is permitted only in system adapters that the profile lists by name together with the access mode. Such an adapter meets the condition of 2.6: it validates a bounded private copy (including the relations between fields), enforces immutability while the buffer is in use, or applies a justified concurrent algorithm. A mutex protects only against participants that follow the protocol.

## B.3. Storage and semantics

Implementation path: checksummed physical store → CID and immutable blocks → manifests/Merkle-DAG → transactional Head/Refs service → durability classes, scrub/repair → retention/GC → assertions and derived indexes. Encryption and key management are designed before confidential data is placed, not added after it has been disclosed.

Chunking is chosen by object type: content-defined chunking, fixed blocks, or a whole small object. Structural sharing reduces rewriting in suitable structures; amplification and its limits are measured separately. The semantic graph may have cycles through EntityID; this does not require mutually recursive hashes of Merkle nodes.

The Head service applies compare-and-swap/MVCC or a defined distributed protocol. A multi-root transaction and snapshot-read have an explicit scope. Write acknowledgment is tied to a durability class. Replication/erasure coding, scrub and repair take into account the independence of failure domains.

Retention is formalized as a registered obligation with a quota, term and owner. Strong/weak/leased references have different semantics; recovery roots, checkpoints and the data of unfinished operations are included in the GC roots. Signed assertions are used where verifiable provenance between trust zones is needed; a local record is not obliged to receive a separate cryptographic signature merely for storage.

## B.4. Execution, supervision and physical control

The baseline organization is a supervision tree with one-for-one/rest-for-one policies and explicit escalation. When a dependency graph is used, recovery cycles are verified separately. The supervisor holds a bounded lifecycle capability, a reserve and a restart budget; detectors for crashes, hangs and incorrect results have different capabilities.

Recovery is specified by a partial order of conditions, not by a single universal sequence: stop accepting new commands of the old epoch; revoke the covered rights; determine the fate of in-flight operations; stop DMA and reset or isolate the device before memory is reused; prepare consistent data and valid rights; check the reserve, the new epoch and the right to launch; publish the ready endpoint. Some steps may be performed in parallel, but their dependencies are proven. An indeterminate result of an external action requires reconciliation, not blind retry.

A persistent actor saves a checkpoint with code/schema versions, inbox/outbox and an effect journal within its contract. The bootstrap/recovery set is available without a functioning main storage service. Relocation requires rebinding of rights and fencing of exclusive resources.

The safety plane uses separate scheduling contexts, pre-allocated reserves, bounded control protocols and hardware-justified interference limits. In the first critical-execution profile, JIT is prohibited; dynamic memory, paging, ARC and host calls are permitted only with proven bounds. Mixed-criticality specifies mode transitions and preservation of control upon failure of the cognitive plane.

Remote capabilities are bound to an audience, scope, epoch and term; the gateway issues a bounded local representation. A short lease is preferred for remote delegation, but its term and renewal rules must not break the agreed autonomous mode. Fencing is checked by the final executor of commands, not only by the router.

## B.5. Boot, Wasm and Marain

The initial update profile is signed manifests and A/B activation with last-known-good, a protected recovery root, a minimum version policy and coordinated migration. Checking for freeze/expiry of metadata takes autonomous operation into account. Emergency recovery power is separated from normal installation and does not bypass accounting.

Wasm is a portable execution format with explicit imports, memory/stack/table limits and accounting of host-function work. WASI/POSIX are not provided automatically. For mutually untrusting components, the runtime is isolated in accordance with the threat model. The AOT compiler operates as a bounded service; trust in its output is included in the TCB or replaced by verification of the output. The compiler is structured as an actor with bounded rights that caches artifacts in storage keyed by the recipe; reference points for comparing runtimes are Lunatic (actors with isolated heaps) and Wasmtime; a project's name is no substitute for measuring its limits, metering and ABI (Appendix G).

The AOT cache stores two different identifiers: the key of the full build recipe and the CID of the resulting artifact. The recipe includes the code, dependencies, compiler/runtime ABI, target features, parameters and security profile. Neither a hit in the CAS nor equality of the source CID grants the right to execute an arbitrary native blob.

Marain is a candidate language for orchestration, supervisors and cognitive components: ADTs, effects, addressable definitions, typed protocols, isolated concurrency. The main backend is the Wasm Component Model via MIND bindings. Details are fixed in [RFC 001](RFC_001_Marain_v0.4.md). Marain is not mandatory for conformance to the Constitution; introduction into the safety plane requires a separate qualified profile. Comparison with the facilities of existing languages is part of the decision stage.

## B.6. Babel / Airlock

Babel is a family of external adapters; Airlock is the profile for separating their authorities. The native object API is not obliged to reproduce POSIX/VFS. TCP/IP is a transport protocol at the boundary, and the POSIX/VFS personality is a separate compatibility service.

Network path and authorities:

| Stage | Minimal power and restriction |
|---|---|
| NIC driver | MMIO/config/IRQ of the specific device and bounded DMA buffers |
| Network stack | Packet/flow endpoints and their quotas; no direct power over the device |
| Policy broker | Grants flow capabilities for a destination, port, term, rate and volume according to the valid policy |
| TLS service | A bounded flow and a key-operation handle with a purpose; the private key is not exported |
| Session parser | Bounded bytes on input and typed messages on output; no arbitrary storage, spawn or device authorities |
| Application gateway | Only the needed service endpoints; re-verification of domain conditions and authorization |

This is a trust map, not a mandatory sequence of six processes. One runtime may combine components of one trust zone, provided the profile does not claim protection between them that is absent. The right to a cryptographic operation is also restricted against abuse.

External medium path: read-only block capability → isolated volume parser → quarantine of objects/projections → authorized import and durability commit → separate indexer. Write/export go through a separate path with separate rights. A projection of a disappearing or mutable source preserves these attributes until import. The parser's rights to LBA requests are restricted to the volume and to read mode.

# Appendix C. Conformance evidence

These are expected results, not verification already performed. For each row, the configuration, assumptions, method, result and unremedied limitations are stated.

| Area | What must be presented for the declared guarantee |
|---|---|
| Authorities | A model of granting, attenuation, transfer and revocation; absence of forgery and of reviving references; accounting for intermediaries; verification of the boot transition |
| Memory and devices | Absence of old CPU/DMA access after the corresponding termination; safe reuse and absence of residual-state disclosure |
| IPC | Defined outcomes of overflow, cancellation, party failure and resource transfer; parsing limits; robustness of validation of shared data |
| Protocol progress | A model of states and wait dependencies; verified properties and explicit assumptions about resources, scheduling and failures |
| Time | Justified bounds for the declared load and configuration; testing under interference as a complement to the justification, not a replacement for the upper bound |
| Supervision | Parties responsible for the lifecycle; budgets and escalation; a working recovery path upon failure of storage, a driver or the recoverer itself |
| Checkpoint / restore | Restoration of a consistent version; a defined fate of messages and effects; no return of revoked rights and no dual exclusive control |
| Storage | Commit model, scopes of multi-root atomicity and reads; behavior on interrupted writes, corruption, out-of-space and concurrent GC |
| Retention | Link between quotas and snapshots, recovery data and confirmed obligations; no unaccounted retention through copying of rights |
| Distribution | Behavior under partition and on restoration of connectivity; impossibility of new commands from the old controlling instance after handover |
| Build and AOT | Pinned inputs; distinction between the recipe and the hash of the result; rejection of an unsuitable or unauthorized native artifact |
| Update | A failure at the preparation, migration, control-handover and completion stages leaves the intended recovery path; the interruption budget is verified |
| Keys | A compromise and recovery protocol with separation of purposes; a defined fate of old signatures, images and autonomous nodes |
| Babel | Damage is limited to the declared session and operations; import does not present a mutable projection as a durable object; write rights do not arise from a read right |
| Audit | Records are sufficient for the declared analysis; integrity and gaps are verified within the model; audit failure has a defined outcome |
| Evolution | A compatibility matrix of interfaces and states; managed migration; binding of evidence to the current configuration |


# Appendix D. Roadmap and transition criteria

The current state of the implementation, the order of work and the point after which parts are developed in parallel are kept in the [ROADMAP](../../ROADMAP.md); the table below sets the functional dependencies of the stages. Constitution stages are numbered 0, I–VII; the labels M0–M7 belong to the Marain RFC and are not mixed with them.

The status of all stages here is **plan; execution has not been verified**. Assurance begins at stage 0 and accompanies every transition. The numbering is uniform for this edition; the language RFC refers to dependencies rather than to incompatible phase numbers from earlier texts.

| Stage | Dependencies | Verifiable result for the transition |
|---|---|---|
| 0. Models and profile | None | Threat/fault model, TCB, objects, budgets, clocks, bootstrap authority, state model and proof plan |
| I. Protected execution | 0 | Bring-up, MMU/IRQ/SMP and accounting; a working DMA boundary before untrusted drivers are admitted; verification of residues and old mappings |
| II. Capabilities, IPC and minimal supervision | I | CSpace/derivation/epochs, bounded queues, ownership and cancellation; MIND IDL; lifecycle owner, recovery reserve and bounded tracing |
| III. Driver vertical slice | II | VirtIO block/net/input; end-to-end block path, device reset, DMA quiescence and restart; then real devices |
| IV. State and recovery | II, block path from III for durability | CID/Head/commit, retention/GC, checkpoint/rebind, recovery roots; recovery without a cyclic dependency on the main service |
| V. Update and distribution | II, IV; transport from III | Full AOT recipe, update authorization, migration, key roles, replication and fencing under partition |
| VI. Safety plane | 0, II, the needed drivers from III | Control actors, authorization of changes to limits, justified deadlines and a degradation mode upon failure of cognitive services |
| VII. User services | Per the contracts of each service | Compositor after display/GPU and fences; Babel after the needed transport; Wasm/Marain after minimal bindings and runtime restrictions |
| Assurance, ongoing | From 0 | Models of critical protocols, implementation analysis, fuzzing, fault injection, recovery drills and verification of the applicability of evidence |

Stage VII is not obliged to wait for the completion of all of V–VI: the dependencies are functional. The network stack does not wait for the semantic index, the Marain compiler may use the local CAS and a host testbed, and qualification of the safety profile follows after a measurable implementation.

**Exit from stage II:** a capability cannot be forged, amplified or revived through a stale handle; copy/move/delegation are distinguishable; revocation has a completion point; boot authority is divided; all queues and allocations are accounted for; enqueue + MOVE does not create two owners; cancellation and failure release or transfer the resource according to the contract; the endpoint generation is checked; the schema is validated outside the kernel; the model of rights amplification, progress and stale access has been verified with declared limits.

**Acceptance of the safety profile:** not merely a successful demonstration of control, but upper timing bounds taking into account interrupts, DMA, host calls, memory release and interference; verified failure of the cognitive plane, budget exhaustion, loss of connectivity, and the impossibility of stopping a physical action by a simple rollback.

**Amendment package:** reason → affected clauses → accepted and rejected alternatives → impact on the TCB, resources and compatibility → migration/rollback → new evidence → activation authorization. This also applies to a change of profile if the prerequisites of guarantees change.

# Appendix E. Integration sources

Files are listed relative to this document. SHA-256 records the bytes reviewed, not authorship or quality. External projects named in the engineering profile are used as research directions; their proofs are not automatically inherited by MIND CORE. Prior art and primary sources are collected in Appendix G; in the RFC they are repeated next to the corresponding decisions.

| Source | SHA-256 |
|---|---|
| [MIND_CORE_Constitution_v1.3ast.md](../const_discusstion_1.3/MIND_CORE_Constitution_v1.3ast.md) | `98e3456e5626a6ca96953807596b0c459b419452faddc5cc93f99e8873b2143d` |
| [MIND-CORE-Конституция-v1.3fab.md](../const_discusstion_1.3/MIND-CORE-Конституция-v1.3fab.md) | `44c4ff4f72a67dbdfdc4e1727825e53cd4619e371f1f9b5df6684bba77884f4b` |
| [const_gem_1.3.md](../const_discusstion_1.3/const_gem_1.3.md) | `41671981d9bb4776e917b2b58b07f198e4c310b99bbca3d1683a29d2f39b30b6` |
| [const_gem_1.3.marain.md](../const_discusstion_1.3/const_gem_1.3.marain.md) | `70a442958b4b622f194c7d9cb5b2ec34e1494f63ce164c317b1fe6df02237324` |

# Appendix F. Rationale and rejected alternatives

This appendix consolidates Volume II of the Fab and Gem editions and table A.3 of the Ast edition. Only Articles 1–12 are normative; here it is explained why precisely this norm was adopted and which alternatives were rejected. "Rejected" refers to the extreme reading of a thesis: the mechanism itself may be retained in the profile in a limited role. Fab's terminological caveat is retained: Linux is a modular monolithic kernel, NT and XNU are hybrids; what is rejected is not the label but the amount of code in privileged mode.

## F.1. Kernel and trusted computing base (Article 1)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| A large privileged TCB: drivers, file systems and network stacks in kernel mode | A driver in kernel mode holds all kernel rights, so its error becomes a kernel error; the amount of privileged code is one factor of the attack surface alongside the authority of services and hardware capabilities (1.6). The cost of IPC remains an engineering question: the L4 family showed that a synchronous fast path makes it acceptable for many workloads, but the threshold is measured for a specific profile (B.2) | 1.1–1.3, 1.6 |
| A physical memory allocator and its allocation policy in the kernel | Policy in the kernel is a DoS surface and a source of non-determinism; it is sufficient for the kernel to guarantee uniqueness of authorities and admissibility of retype | 1.4, 1.7; B.1 |
| "Exactly 3–5 kernel functions", "the kernel does not manage memory at all" | An arbitrary count does not define a minimal TCB; exception routing, SMP, IOMMU, object lifecycle and tracking of rights derivation are inevitably privileged | 1.1–1.2 |
| User-mode drivers as sufficient isolation | DMA without a hardware boundary bypasses the MMU; the death of a driver does not stop DMA and does not reset a hung device | 1.5, 6.3; B.4 |
| A small kernel means a small TCB | The trusted computing base includes the bootloader, firmware, IOMMU configuration, toolchain, runtime and hardware | 1.6; B.1 |
| Isolation and quotas make DoS impossible | Quotas limit damage within the model; accounting is needed for the work of intermediaries, shared hardware and recovery reserves | 3.13, 5.1–5.5 |

## F.2. Interaction (Article 2)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| UNIX byte streams as a universal interface | A custom parser in every program; no schema, version, size limits or failure semantics | 2.3–2.5 |
| Shared mutable memory with mutexes as the primary mechanism | Races, deadlocks, TOCTOU on shared buffers | 2.1, 2.6, 2.11 |
| The classic Erlang actor model "in pure form" | Does not provide static message-ordering contracts; isolation excludes only data races | 2.9–2.10; B.2 |
| Actors exclude all races and deadlocks | Logical ordering races, stale replies, cyclic waits and starvation remain | 2.9–2.10 |
| "Pure" zero-copy through remapping; O(1) cost | Unmap on SMP is paid for with a TLB shootdown; the cost depends on the number of pages and devices; a small message is faster to copy | 2.9; B.2 |
| Ring buffers eliminate mapping changes | They reduce them on the hot path but do not eliminate the mapping lifecycle and robust data validation | 2.11; B.2 |
| Type checking of a single channel proves progress | Progress of a composition requires a model of waits, queues, cancellation and failures | 2.10 |

## F.3. Authorities (Article 3)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| ACL, UID/GID and root | Ambient authority: a program acts with all the user's rights, so a single error or a forged request obtains all of them; a service that checks rights by the caller's identity becomes a confused deputy when the request is not bound to the permitted scope of the action (3.8); default rights are wider than the task needs (a calculator does not need personal photos) | 3.1, 3.7–3.8 |
| A complete ban on identity | Identity is needed when granting rights, in auditing and in provenance; only bypassing authority checks on its basis is prohibited | 3.10, 10.3 |
| Cryptographic local capabilities (Macaroons and similar) | A token is copyable as bytes and hard to revoke; observable completion of revocation requires online checking; within one node, a protected kernel table is simpler and stronger | 3.2, 3.6; B.1; cryptography is for remote rights (7.5) |
| Capabilities by themselves eliminate the confused deputy and limit damage to "the contents of the pocket" | Available intermediaries, proxying, further granting of power and effects already produced are taken into account | 3.8–3.9 |
| Any revocation is cascading and instantaneous | A scope, a completion point, the fate of operations in progress and a distinction between derived and independently granted rights are needed; on an autonomous remote node, revocation depends on the protocol and connectivity | 3.5–3.6, 7.5 |
| An init that permanently holds capabilities to everything | This is root under another name | 3.7, 3.12 |
| The presence of an effect handler proves the presence of a right | Static admissibility differs from valid authorization, which may change after the build | 3.11, 11.7 |

## F.4. Storage (Article 4)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| A hierarchical VFS as the native model | One path per file, loss of versions on overwrite, manual deduplication, rights bound to the path | 4.1 |
| "Pure" CAS without a Head/Refs layer | Without it the system has no notion of a "current version", and concurrent updates are unresolvable | 4.3–4.4 |
| CAS without chunking | Editing a byte duplicates the entire object | B.3 |
| CDC guarantees O(log n) writes when a byte is edited | The cost depends on the chunking scheme, the index structure and the nature of the edit; the limits are set by the storage specification | B.3 |
| "Eternal history" without retention | The cost of retention is not accounted for; this is self-deception | 4.5, 4.11 |
| Holding a capability as the sole basis for an object's liveness | The right to read and the obligation to retain are distinct; recovery images and confirmed obligations must not be lost along with a temporary handle | 4.11 |
| A hash as meaning, right or confidentiality | A hash verifies correspondence to bytes; predictable content can be guessed by dictionary; meaning is carried by assertions with provenance | 4.6–4.7 |
| A signature on every local object | Excessive; a signature is needed where provenance between trust zones is verified | B.3 |
| A search index as a source of truth or authority | An index is derived and recoverable; it does not bypass access checks and does not permit deletion of the source data | 4.6, 4.12 |

## F.5. Time, failures and recovery (Articles 5–6)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| Average latency as a deadline guarantee | Upper bounds are needed for the specific platform and load, taking into account shared caches, buses and interrupts | 5.3 |
| Absence of a tracing GC or the choice of Perceus/ARC guarantees hard real-time | The profile is obliged to bound all significant pauses, including deallocation cascades and host calls | 5.3–5.4; B.4; RFC §6 |
| "Crashed — restart it" | Without revoking rights, changing the epoch, cancelling in-flight operations and stopping DMA, a restart destroys state | 6.3–6.5; B.4 |
| Any crash leads to transparent recovery of the actor | A checkpoint, code, authorities, resources and reconciliation of external effects are required; sometimes only a cold start or degradation is admissible | 6.10–6.11 |
| A mandatory single OTP tree for all components | The tree is the baseline profile; the norm requires a lifecycle owner and an escalation path, permitting a graph with cycle checking | 6.8; B.4 |
| The system will detect a break-in and instantly restart a clean component | A compromised component may keep running; recovery does not undo a leak, a wrong answer or an external action | 6.7 |
| A restart or rollback undoes an external action | It does not; an indeterminate outcome requires reconciliation | 6.6, 9.9 |

## F.6. Distribution (Article 7)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| Locality is merely a transport detail | Common types do not eliminate differences in latency, partial failures, placement and indeterminacy of the result | 7.1, 7.7 |
| Copies on a shared resource as independent replicas | Common causes of failure: power, controller, software version, keys | 7.3 |
| A mandatory multikernel | A candidate, not a norm: the choice between a shared SMP kernel, a multikernel and a mixed variant is made on the basis of the coherence and failure model | B.1 |

## F.7. Provenance and update (Article 9)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| The hash of a Wasm module as a sufficient AOT cache key; the cache is "free" | A full recipe is needed: dependencies, toolchain, target, runtime/ABI, profile; the provenance of the native artifact is verified | 9.7–9.8; B.5 |
| An AST hash eliminates version conflicts | It names the code precisely, but does not prove compatibility of contracts and environment | 9.7; RFC §8 |
| A signature proves correctness | Only authorized provenance | 9.2 |

## F.8. Languages and Babel (Article 11)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| A single Babel actor or a monolithic gateway | A single point of compromise combining raw input, keys and broad rights | 11.11; B.6 |
| A fixed chain of N processes | Decomposition follows the threat model; merging stages is admissible provided the restrictions are preserved | 11.4; B.6 |
| A safe language or Wasm exempts from checks | It does not: TOCTOU, side channels and logical errors remain | 11.2, 10.5 |
| Gleam as the high-level language (edition 0.1) | Its targets are BEAM and JavaScript, not Wasm | B.5; RFC §14 |
| File-system emulation as "moving the magnetic head" | Translation reduces to namespace operations over LBA requests to the block service | B.6 |

## F.9. Evolution (Article 12)

| Rejected alternative or thesis | Why | Adopted norm |
|---|---|---|
| An optional field is automatically compatible | Compatibility covers semantics, rights, errors and resource-and-timing conditions | 12.7 |
| Expiry of a calendar deadline permits switching an interface off | Managed migration and detection of remaining dependencies are required | 12.8 |
| Eternal compatibility in the kernel | Old versions live in adapters with a lifecycle | 12.4 |

# Appendix G. Prior art and primary sources

The table consolidates the "required reading" list of the Fab edition and the primary sources of Appendix D of the Ast edition. A specific decision is taken from each system; its proofs, ABI and limitations are not automatically carried over to MIND CORE (12.1). Links are given only for sources checked during preparation of the 1.3 editions.

| Article / section | System or document | What is used | What we do not inherit |
|---|---|---|---|
| 1, 3; B.1 | seL4 — [Verification](https://sel4.systems/Verification/), [Capabilities](https://docs.sel4.systems/Tutorials/capabilities.html), [Untyped](https://docs.sel4.systems/Tutorials/untyped.html) | Untyped/retype, CDT revocation, an explicitly declared proof scope; a reference point for verification cost (on the order of 20 person-years for ~10 KLOC) | seL4's proofs and its platform assumptions |
| 1, 3, 6 | KeyKOS / EROS | Capability OS, orthogonal persistence, narrow managers instead of root | Automatic persistence of every component |
| 1 | The L4 family; QNX | Synchronous IPC fast path; a message-passing microkernel in industrial operation | The specific ABI and thread model |
| 1–3 | Fuchsia / Zircon; Genode | Living capability systems; Genode's component tree and budgets | Their object models as a norm |
| 1; B.1 | Redox OS | Cross-checking of the engineering decisions of a Rust microkernel | — |
| 2; B.2 | Singularity / Midori; Joe Duffy's essays on Midori | Channel contracts as statically verifiable state machines; buffer ownership modes | A single managed language for the whole system |
| 4; B.3 | Venti (Plan 9); IPFS (multihash, CID); Perkeep | CAS, self-describing identifiers, content-defined chunking, trees as views | The global network and its access model |
| 4 | [RFC 6920 — Naming Things with Hashes](https://www.rfc-editor.org/rfc/rfc6920.html) | The limits of hash identity; dictionary guessing of predictable content | A hash as a right or as confidentiality |
| 4.13 | [Git — Hash Function Transition](https://git-scm.com/docs/hash-function-transition) | A hash-function migration plan for objects and references | — |
| 6; B.4 | Erlang/OTP | Supervision trees, restart strategies, escalation, hot code upgrade with state migration | Absence of static contracts; unbounded mailboxes |
| 6.10; B.4 | Phantom OS; EROS | Orthogonal persistence as a profile | A memory dump as a portable checkpoint |
| 7; B.1 | Barrelfish | Multikernel: cores do not share OS state; a single IDL with differing semantics | Mandatory multikernel |
| 9; B.5 | [The Update Framework — Overview](https://theupdateframework.io/docs/overview/) | Protection against rollback and freeze, key roles and rotation, metadata expiry | — |
| 9.8; B.5 | [Wasmtime — Engine::precompile_compatibility_hash](https://docs.wasmtime.dev/api/wasmtime/struct.Engine.html#method.precompile_compatibility_hash) | Compatibility of an AOT artifact depends on the engine configuration, not only on the module hash | — |
| 11; B.6 | OpenSSH privilege separation | Separation of the untrusted-input parser, keys and the privileged effect | — |
| 11; B.5 | [Component Model — WIT](https://component-model.bytecodealliance.org/design/wit.html); [WebAssembly — Security](https://webassembly.org/docs/security/) | Candidate representation for MIND IDL; sandbox boundaries | Protocol behavior and progress from WIT alone; zero leakage from the sandbox |
| B.1 | [Rust — Unsafe Rust](https://doc.rust-lang.org/book/ch20-01-unsafe-rust.html) | The contract of the unsafe boundary; safe interfaces depend on the invariants of unsafe code | Identity between language ownership and cross-domain authorization |
| B.5 | Lunatic | Reference actor-based Wasm runtime with isolated heaps, for comparison | Fitness for critical execution without measurements |
| B.5; RFC §§4, 6, 8 | [Unison — The big idea](https://www.unison-lang.org/docs/the-big-idea/); Koka; [Perceus](https://www.microsoft.com/en-us/research/publication/perceus-garbage-free-reference-counting-with-reuse/); Pony | Hash addressing of definitions; algebraic effects and handlers; ARC with reuse; reference capabilities iso/val/ref | A ready-made backend, runtime or timing guarantees |
| 3 (outlook); B.1 | CHERI / Morello | Hardware capabilities as the target mechanism for the capability model | Seamless porting without new ABIs and evidence |

# Appendix H. Changes in edition 1.6

| What | Change | Basis |
|---|---|---|
| Document status | Full edition based on 1.4; editions 1.4 and 1.5 moved to `archive/` | Review, §6 item 1 |
| Normative status of appendices | The 1.4 rule is restored: Articles 1–12 are normative; Appendices B–E are profile, evidence, plan and sources (1.5 declared the profile normative) | Review, §6 |
| Requirement IDs | Stable IDs `MC-<article>.<clause>` | Review, §6 item 3 |
| Normative language | Definitions of MUST / MUST NOT / MAY and the precedence rule kept | Review, §6 item 4 |
| 2.6, `SHARE_RW` | The 1.5 addition accepted in refined form: restriction by profile, requirements on the adapter, link to 2.11; the concrete restriction is in B.2 | Review, §5 |
| 6.10, checkpoint | The 1.5 addition (queue reconciliation) accepted as a requirement on the profile's protocol; domain logic stays out of the kernel | Review, §5 |
| Articles 2–6 | The 1.5 abridgements are not accepted: the full 1.4 texts apply | Review, §3 |
| Appendix D | Link to the ROADMAP; stages 0, I–VII separated from M0–M7 | Review, §6 |
| Appendix F.1, F.3 | Categorical rationales toned down: attack surface, cost of IPC, confused deputy | Review, §9 item 6 |
| Integration sources | The `const_discusstion_1.3` files are not part of the repository; the links in Appendices A and E are kept as historical, the checksums fix the bytes that were reviewed | — |

Articles 1–12, apart from the additions to 2.6 and 6.10, are identical to edition 1.4.

**End of edition 1.6.**
