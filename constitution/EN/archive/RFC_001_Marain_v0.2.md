# RFC 001 — Marain: the MIND CORE contract execution language

**RFC revision:** 0.2, a development of the original RFC 001; this is not a version of a finished compiler.  
**Date:** 20 September 2026.  
**Status:** a worked-out draft for discussion and prototyping; the decisions below are proposed for adoption, no implementation or proofs are claimed.  
**Normative basis:** [MIND CORE Constitution v1.4](MIND_CORE_Constitution_v1.4.md).  
**Original RFC:** [Marain, original draft](../const_discusstion_1.3/const_gem_1.3.marain.md).  
**Affects:** MIND IDL, runtime, capabilities, supervision, storage, build, updates and development tools.  
**Initial purpose:** cognitive services, orchestration and bounded supervisors. Critical physical control requires separate qualification.  
**Revision 0.2 (20 September 2026, review):** fixed the notation of the owning handle in the WIT example (§11); the result model of §4.3 aligned with §7.5; side channels included in the threat model boundaries (§2); `List<T>` added to Core (§3.1); link to the renamed Constitution file.

## 1. Abstract and scope of the document

Marain is a draft of a functional, statically typed language with explicit effects, content-addressed definitions, isolated actors and message contracts. The execution target is the Wasm Component Model via restricted MIND bindings. Text sources remain a full-fledged format for development and exchange; the addressable internal representation provides exact pinning of dependencies.

The language should help express and check the requirements of the Constitution, while the trusted OS mechanisms enforce authority, isolation, revocation and resource limits when executing any component, including one obtained from another language. Successful compilation does not confirm a valid authorization, the physical safety of a command, or meeting a deadline on arbitrary hardware.

**MUST**, **MUST NOT**, **MAY** have normative meaning within the proposed Marain profile. Once the RFC is adopted, conformance to it is claimed separately from the OS's conformance to the Constitution. In case of conflict, the Constitution prevails. The syntax of the examples is a draft: until a compiler exists, the examples are not presented as an executable verified program.

### 1.1. Goals

- Express external actions in types and provide no ambient I/O, global service lookup or automatic inheritance of authority.
- Make ownership modes, errors, quotas, lifecycle and the message protocol part of the contract available to the developer.
- Separate effect requirements, handler binding and the actual authorization of an action.
- Provide exact dependency identity, reproducible builds in a fixed profile, state migration and verifiable artifact provenance.
- Allow starting with a small compiler and a host test bench without requiring the whole OS to be ready.

### 1.2. What is not among the promises of the first version

Marain does not replace the kernel, drivers, hardware isolation or the domain checks of the safety plane. It does not promise the absence of all deadlocks, termination of any program, bit-for-bit reproducibility on all ISAs, automatic persistence, exactly-once for an arbitrary external device, or hard real-time from a single choice of ARC. A native POSIX personality is not part of the standard library; access to compatible services is possible via an explicit Babel contract.

## 2. Execution model and allocation of guarantees

Execution path:

```text
Text → name resolution → typed Core IR → CodeID
                         ↓ type/effect/ownership/protocol checks
                    Wasm + WIT + MIND manifest
                         ↓ verification and authorized binding
                    actor runtime / host bindings
                         ↓ protected handles and IPC
                    kernel mechanisms → services / control components
```

WIT describes component types and interfaces, but not protocol behavior. Therefore the MIND contract additionally contains a state machine, resource constraints and failure rules. This is a design extension around the Component Model, not a claim that WIT already provides these properties. [Primary source: WIT Reference](https://component-model.bytecodealliance.org/design/wit.html).

| Layer | Checks or enforces | What it does not confirm by itself |
|---|---|---|
| Compiler | Types, effects, absence of use of a moved value, local protocol order in the model | Current right, behavior of an untrusted counterparty, termination of the whole system |
| Loader/linker | Interface compatibility, permitted imports, completeness of the binding plan, provenance and profile | Indefinite retention of authority once granted |
| Runtime/host adapter | Input validation, dynamic protocol states, handles, accounting and delivery of failures | Correctness of the kernel, driver or physical device |
| Kernel/trusted mechanism | Unforgeability of rights, memory boundaries, completion of MOVE/revocation, enforced budgets | Meaning of application messages and correctness of the world model |
| Supervisor/services | Grant policy, recovery, commit/durability, fencing and domain checks | Impossibility of a silent wrong result |

The threat model includes untrusted messages, a different language on the counterparty's side, changes of rights after launch, failure at any point of an operation, resource exhaustion and partial connectivity. Microarchitectural side channels and leakage through execution time or resource consumption are not closed by the language; their profile and residual risks are set by the Constitution (10.5). The correctness of the compiler, runtime, bindings and kernel is included in the TCB of those properties that depend on them. Wasm validation restricts module behavior within the sandbox, but does not make all objects inside its linear memory safe. [WebAssembly Security](https://webassembly.org/docs/security/).

## 3. Minimal language core

### 3.1. Data, computation and errors

Initial Core: strict left-to-right evaluation order; lexical scopes; immutable values by default; first-class functions; parametric types; records; algebraic data types; exhaustive `match`; explicit recursive definitions. A function and a closure have a latent effect set; captured resources are included in ownership analysis.

Primitives: `Unit`, `Bool`, fixed-width integers, `Bytes`, UTF-8 `String`, `Option<T>`, `Result<T,E>`; the Core standard library adds the length-bounded sequence `List<T>`. Integer overflow does not depend on debug/release mode: an ordinary operation ends with a declared trap; the library `checked_*` return `Result`, `wrapping_*` have explicitly modular semantics. External strings are validated as UTF-8; binary data is not automatically interpreted as text. Unicode normalization of data is not performed implicitly.

Mutable values are allowed within a scope of exclusive ownership. There are no mutable global objects with implicit external access. General recursion is allowed in the ordinary profile and is bounded by execution resources; its presence precludes inferring termination from type checking alone. In a profile with a timing guarantee, recursion requires a separate upper bound or is prohibited.

Expected failures are `Result`: denial of a right, revocation, queue overflow, publication conflict, unavailability. A trap is intended for a violation of an execution invariant, an arithmetic fault, an out-of-bounds access and unhandled memory exhaustion. A trap terminates the current actor, passes a structured failure to the supervisor and starts accounted cleanup. It is not given to the client as proof of the absence of an external effect.

### 3.2. Signatures and purity

```marain
fn checked_sum(a: U64, b: U64) -> {} Result<U64, Overflow>
fn load(key: CID) -> {StoreRead} Result<Bytes, ReadError>
fn map<T, U, e>(xs: List<T>, f: fn(T) -> {e} U) -> {e} List<U>
```

`{}` means the absence of observable effects in the language model. A pure function still consumes CPU/memory, may fail to terminate, or may terminate with a trap. Given identical inputs and the same semantics version, its successfully obtained result is deterministic. Resource cost and successful termination are checked separately.

The effect set is inferred within a definition, but an exported interface must state it explicitly. The polymorphic row `{Log | e}` means `Log` plus the effects of parameter `e`; binding substitutes a concrete row. An effect variable cannot be a way to hide an undeclared import. The MVP uses monomorphization of exports so that the ABI does not depend on the language's generic types.

Floats are not part of the minimal deterministic Core. The `f32/f64` extension fixes the rules for NaN, rounding and optimizations; cross-platform bit-for-bit equivalence requires a separate profile. A memory address and hash-table order are not observable properties of a portable pure program.

## 4. Effects, handlers and authority

### 4.1. Three different entities

An **effect** describes a class of action. A **handler** defines the interpretation of an operation. A **capability** permits specific access in the running system. For example, `Net` in a signature does not specify a permitted address, term or volume; these are properties of the passed flow-capability and of the checking service.

A handler may refuse, work with a test model, or call a service via a narrow authority. A right does not arise from an `effect` declaration, from the presence of a function with a suitable signature, or from a change to the manifest. A real system action requires a chain of current authorization up to the executor. A handler using its own right is an explicitly authorized intermediary and must constrain the client's request.

Formal rule for `handle f with h`: the effects of the handled operations are removed from the row of `f`; the effects of the body of `h` and of the functions it calls are added to the resulting row. The handler body itself does not receive a magical right to perform the intercepted effect. Reaching a lower handler requires an explicit binding, not an implicit repeated lookup of the same name.

### 4.2. Semantics of handlers

Deep handling with a one-shot continuation is proposed: `resume` consumes the continuation and may be called at most once. On resumption the handler remains installed for the continuation of the protected computation. A repeated `resume`, copying a continuation with affine resources, and serializing a continuation are prohibited. Zero calls of `resume` terminate the corresponding scope with a defined result or cancellation and accounted cleanup; a blocked task cannot be silently lost.

In the MVP a continuation does not leave the dynamic scope of the handler and is not stored in a mailbox. Asynchrony is performed by runtime primitives with separate tickets, not by arbitrarily holding a language continuation. Multi-shot handlers and first-class continuations are deferred: they complicate ownership and can repeat external effects.

### 4.3. Launch and revocation

Before an actor is launched, the binding plan MUST cover all mandatory effects of the entrypoint. A statically known absence of a handler is a link-assembly build error; a dynamically loaded component without a complete plan is rejected before the entrypoint executes. A library with abstract effects may be compiled separately.

Spawn receives an explicit image, init data, child budget, lifecycle scope and bindings. The parent's dynamic environment, its handlers, capabilities, clock and stdout are not inherited. A handler in another domain is represented by a restricted endpoint; its closure is not forwarded together with pointers or local handles. Binding an endpoint requires authority to delegate or to provide such a service.

Each protected operation checks the current right with the authoritative executor. A host adapter may cache a decision only under a protocol that preserves the promised revocation semantics. Once revocation completes, new operations are rejected; for operations already started, the contract reports `Completed`, `NotStarted(CancelledBeforeCommit)` or `OutcomeUnknown` and the means of reconciliation (§7.5). Local visibility of a `Cap` value in a program does not prove that the right is still valid.

## 5. Ownership and resource types

| Form | Meaning | Transfer and copying |
|---|---|---|
| `iso<T>` | An exclusively owned data graph | MOVE consumes the value; implicit copying is prohibited |
| `val<T>` | Deeply immutable data without nested live authority | Copying is allowed; local sharing of the representation is an optimization |
| `ref<'s,T>` | A temporary borrow within scope `'s` | Does not leave the scope, is not serialized, not forwarded and does not survive suspension |
| `Cap<I>` | An opaque affine authority over an interface | Copy/restriction/delegation only via a permitted operation; checked at execution |
| `Endpoint<P,S>` | An affine end of a channel of protocol `P` in state `S` | A transition returns a new state; an alias of the old state cannot be used |
| `MemoryObject<mode>` | An external managed buffer with a declared access mode | COPY/MOVE/SHARE_RO/LEASE semantics are enforced by the runtime and kernel |

`val` is not a freeze of an arbitrary graph with capabilities: such packaging could implicitly multiply authority. A record with a `Cap` remains affine. A wrapper does not allow obtaining a handle from a `U64`, bytes, a CID or a snapshot. Handle numbers passed via the ABI are resolved only in the recipient's protected table with a check of type, owner and generation.

`freeze(iso<T>) -> val<T>` is allowed only for suitable graphs after all write rights have ceased. `thaw` requires uniqueness or creates a copy. A MOVE of a heap object between Wasm instances may physically copy bytes and free the original; exclusive ownership semantics do not promise zero-copy. Genuine cross-domain zero-copy uses a `MemoryObject` for which the absence of stale CPU/DMA accesses has been proven. An ordinary WIT `list<u8>` does not provide such a property.

Dropping a `Cap` releases the local reference; this is not equivalent to revoking all derived rights, deleting data or cancelling a hold. An affine value may be left unused, but the release of its external resource is performed per the contract and quota. Unbounded user code, I/O and blocking actions in implicit destructors are prohibited. Closing with an external effect is an explicit operation with a result.

`SHARE_RW` is not part of the safe Core MVP. Its later profile requires a defined memory model, atomicity, synchronization and a TOCTOU check. `LEASE` is revocable access to an object, not an ordinary language reference: expiry of the term ends access via a trusted mechanism before the memory is reused.

## 6. Resources, memory and time

### 6.1. ARC and the scope of the initial implementation

The baseline candidate is reference counting with liveness analysis and reuse of a unique representation, inspired by Perceus. This is a choice of implementation direction, not a claim of implemented compatibility with Koka. The Perceus paper describes reference counting and reuse; an upper time bound for a specific system requires separate justification. [Perceus: Garbage Free Reference Counting with Reuse](https://www.microsoft.com/en-us/research/publication/perceus-garbage-free-reference-counting-with-reuse/).

In the MVP, graphs of owning heap references are acyclic: there is no operation to tie a recursive value or to store a borrow inside its own owner. Recursive functions are represented by static references to definitions; actor communication cycles do not become cyclic heap references. User graph structures use node identifiers and explicit storage. Admitting strong cycles later requires a separate region/cycle protocol; they are not left as a leak "by default".

Decrementing a counter may trigger a long cascade of deallocation. The ordinary profile allows staged cleanup within the quota; deferred objects remain accounted until the memory is actually freed. The size of the cleanup queue is bounded and reserved in advance. Exhaustion leads to backpressure or allocation failure; there is no hidden unbounded cleanup list.

### 6.2. Three profiles

| Profile | Constraints | Level of promises |
|---|---|---|
| `core` | Limits on heap/stack/mailbox/actors/handles; preemption and accounting of host work | Isolation and finite budgets; no timing guarantee |
| `replay` | `core` + fixed semantics and recording of nondeterministic inputs | Replay within the declared model given a complete log |
| `rt` | AOT, JIT prohibited, preallocated/bounded allocations, recursion and deallocation limits, bounded host calls and platform analysis | Only explicitly qualified deadlines of the specific profile; deferred beyond the MVP |

Quotas are required for CPU, heap, stack, tables, queues, number of actors, capability slots, compilation, storage and recovery. "Fuel" metrics and instruction counts are not equal to wall-clock time: host calls, interrupts, caches and buses are accounted separately. The scheduler must preempt computation without depending on a voluntary `yield` by untrusted code; a host call has bounded duration or a cancellable asynchronous contract.

The rights `Clock`, `Random`, `Spawn`, `Log` and `Store` are granted explicitly. Limiting the duration of an operation uses the runtime's monotonic clock; this does not give the program the right to read the clock as a source of information. Calendar timestamps arrive via a separate interface.

## 7. Actors, channels and progress

### 7.1. Actor model

An actor has an entrypoint, a local heap, a bounded mailbox, the current epoch, a binding plan and an assigned supervisor. Within a single actor a message handler does not execute reentrantly; concurrent child work is created explicitly and accounted for. The runtime may co-locate actors of one trust zone in one domain, but for untrusted participants it uses the declared protection boundary. A new actor is not by definition equal to a new hardware address space.

Each spawn resides in a lifecycle scope. Termination of a scope cancels work not yet started, determines the fate of descendants and awaits their accounted termination under a bounded protocol. A long-lived actor is handed over to another supervisor by an explicit authorized operation. A "detached" unaccounted task does not exist.

### 7.2. Full channel contract

The contract contains the `ProtocolID`, data version, states and transitions, maximum message size and nesting depth, permitted capabilities, ownership mode, queue capacity, and the rules for ordering, deadline/cancellation, retries, failure and epoch change. For a stream, frame size and window are specified; the number of frames may be unbounded with bounded buffers.

Order is defined per specific channel/session: for a basic ordered channel, accepted messages are delivered in the order of its sequence numbers. There is no total order across messages of different channels. Selective receive with unbounded queue scanning is not part of the MVP; filtering is bounded by the protocol and budget.

Example of a request, client-side view:

```text
Idle --submit(operation_id, request) accepted--> Waiting
Idle --submit rejected before commit----------> Idle + original request
Waiting --reply(completed | rejected)---------> Idle
Waiting --cancel requested-------------------> Cancelling
Waiting/Cancelling --timeout or peer failure--> Uncertain(operation_id)
Cancelling --cancelled-before-commit----------> Idle
Cancelling --completed-----------------------> Idle
Uncertain --reconcile(completed | cancelled)---> Idle
Uncertain --reconcile(unknown)-----------------> Uncertain
any state --session close---------------------> Closed + accounted fate of resources
```

`Uncertain` is a mandatory part of protocols with external effects if it cannot be proven that the action was not accepted. The client may stop waiting, but the record of the unresolved operation is handed to the party responsible for reconciliation. The quota of such records is taken into account when accepting new operations. The transition to `Closed` by itself does not cancel the effect.

### 7.3. MOVE and the point of acceptance

The first transport is local bounded IPC. `try_send` has exactly two defined outcomes: `Accepted(receipt)` or `Rejected(original_payload, reason)`. When the queue is full, an immediate rejection returns the payload and the nested rights; waiting for free space is expressed as a separate bounded operation. The runtime reserves the queue and descriptors, checks the modes and atomically commits the enqueue together with the ownership transfer.

Before commit the sender remains the owner; after commit the accounted queue becomes the owner, then the recipient. The sender loses its former access; the existence of a receipt confirms acceptance, not the application effect. If the recipient dies, the queue is handed to the recoverer or released per the contract. On a crash between commit and reporting the result, the runtime/ticket log allows the owner to be determined; unavailability of the result does not permit returning a second copy of an exclusive resource.

Distributed delivery does not pretend to be a local MOVE. The initial remote profile transfers serializable data and restricted remote authorities via a gateway; exclusive devices are switched over via fencing. When the result of a remote send is unknown, ownership is held by an escrow/gateway until definite resolution, or such a transfer is not supported by the profile. A network timeout has no semantics of automatic return of ownership.

### 7.4. What protocol typing checks

The compiler checks the admissibility of a local transition and double use of an affine endpoint. The runtime checks the messages actually received from an untrusted counterparty, the epoch, limits and state. The protocol version and hash are part of the component's link to the contract.

Absence of deadlocks in a composition requires separate analysis of the wait graph, queues, priorities, cancellation and failures. For the MVP, it is prohibited to claim deadlock freedom from a session-type check alone. The contract of a critical function contains the necessary assumptions about fairness and maximum load; resource failure is a state of the model, not an excluded accident.

### 7.5. Unified operation result model

For an external effect the library distinguishes `NotStarted(reason)`, `Completed(value)` and `OutcomeUnknown(operation_id, cause)`. A completed refusal by a domain service may be the value `Completed(Result::Err(...))`: the transport must not confuse it with an unaccepted request. A partially performed action is represented by an explicit application result describing the consequences or by an undetermined outcome, but not by `NotStarted`.

| Cause | Permissible result value |
|---|---|
| Denied, Revoked, QuotaExceeded, QueueFull, TooLarge | `NotStarted`, if the executor confirms the refusal before commit of the corresponding action; after commit the result protocol applies |
| Malformed, ProtocolViolation, IncompatibleVersion, StaleEpoch | The unaccepted request is rejected; the fate of the session's already accepted requests is resolved separately |
| DeadlineExceeded, Disconnected, PeerFailed | `NotStarted` only with confirmed absence of commit; otherwise `OutcomeUnknown` |
| CancelledBeforeCommit | Confirmed absence of effect; the fate of the payload corresponds to the transfer state, not to the client's assumption |
| Executor response with an established outcome | `Completed` with the application result and the operation identifier |

Enqueue, Head publication and a physical action may have different commit points. A queue receipt records only the first of them. The library does not provide automatic retry for `OutcomeUnknown` without a declared idempotency/reconciliation protocol. During recovery, an already known completed result is not downgraded to "not started" due to the loss of a temporary handle.

## 8. Addressable code, dependencies and AOT

### 8.1. What is hashed

Using hashes instead of mutable definition names preserves the idea of the original RFC; a similar separation of name and identity is used in Unison. [Unison: The big idea](https://www.unison-lang.org/docs/the-big-idea/). Neither a hash nor the coexistence of two versions proves compatibility of their types, protocols or state.

The proposed `CodeID` is computed over the canonical **typed Core IR after name resolution**, not over raw text or optimizer-dependent machine code. The input includes:

1. The domain separator `marain.core`, the codec version and the language semantics version.
2. The definition, type, effects, ownership constraints and protocol identities.
3. Literals with exact encoding and references to the specific `CodeID`/`TypeID` of dependencies.

Whitespace, comments, file location and local binder names are excluded; local binders are encoded as indices. Renaming an external alias does not change the resolved graph. The order of fields, variants and computations is preserved if it is semantically significant. An annotation affecting the contract is included in the hash; diagnostic documentation is stored separately. Programs equivalent in meaning are not required to have the same `CodeID`.

Core IR is hashed before backend optimizations. The CID algorithm and format are specified explicitly; unknown versions are rejected. Test vectors define the encoding of numbers, strings, types, effect rows and indices so that independent implementations produce the same result.

### 8.2. Recursion, names and storage

Mutual recursion is represented by an explicit `rec` group: internal references use member indices in the preserved declaration order, external ones use finished CodeIDs. The hash is computed over the whole group; a reference to a member contains `(groupCID, memberIndex)`. This breaks the cyclic computation of hashes. Reordering members may change the hash; canonicalization of the graph up to all permutations is not promised. Changing a member changes the identity of the group.

`SourceCID` identifies source text; `CodeID` a resolved definition; `ArtifactCID` the specific bytes of a build result. Package names and versions are mutable human-readable aliases with provenance. The lock manifest pins exact definitions, types and the name resolution scheme. An alias conflict is resolved explicitly; the "latest" library is not chosen at launch.

The CAS does not grant access to closed source or the right to execute it. Retention of code, dependencies, source maps and schemas is tied to supported releases and checkpoints. Without retention an old CodeID may become unavailable; the promise "old code always works" is not accepted.

### 8.3. Build and launch are different decisions

```text
BuildKey = H(canonical(
  root CodeIDs + dependency closure + MIND/WIT contracts,
  compiler/toolchain identities + options + semantic profile,
  target ISA/features + runtime/ABI + security/metering profile,
  declared build inputs and relevant environment
))
ArtifactCID = H(exact output bytes)
```

The cache links a BuildKey to an ArtifactCID and verifiable provenance. Host contracts are part of the recipe; current secrets and live capabilities are not serialized into it. All influencing generators and build scripts have fixed inputs and bounded effects. Build time does not affect the reproducible bytes unless it is declared as an input.

At launch, the artifact's authorization, suitability for the platform/runtime, conformance of imports, current rights and resource admission are checked separately. A recipe hash does not prove the correctness of the compiler; a provenance signature does not replace TCB analysis. Substitution of a native blob under the same key is rejected by checking the bytes and provenance. Changing runtime/target/profile requires a new compatible artifact, not reliance on the same CodeID.

## 9. Storage, checkpoints and updates

Standard bindings separate operations: reading by CID via a read-capability; creating an immutable object via a storage budget; publishing a Head via a separate update-capability and an expected version; retention via a retention contract. Obtaining a CID does not grant the right to read. `put` reports the durability achieved; `publish` accepts only data satisfying the root's contract. A compare-and-swap conflict is returned explicitly; several Heads are updated only within a declared transaction.

A persistent actor declares a serializable state type, a SchemaID, a checkpoint point and a recovery contract. A snapshot contains CodeIDs/protocol versions, logical state, consistent inbox/outbox offsets or equivalent records, operation IDs and unresolved effects. The specific format depends on the service, but the consistency boundary is mandatory. An arbitrary dump of the stack, a continuation or Wasm memory is not considered a portable checkpoint.

Capabilities are replaced by **descriptions of needs** for rebind: the logical resource, interface, constraints and binding identifier. Such a description is not a bearer token. Restore checks current policy, revocation, epochs and fencing; it issues new permitted handles or returns a definite refusal. Cloning a snapshot does not clone exclusive authority.

An exactly-once effect guarantee is possible only if the executor participates in a protocol with an operation ID and durable accounting of the result/state. An outbox by itself does not guarantee this for an arbitrary network or actuator. On timeout, `OutcomeUnknown` remains; retry, reconciliation and compensation are distinct explicitly authorized operations.

Update: preparation of new code and schemas → verification of migration and reserves → coordination/stopping intake of the old session → committing state → transfer of authority and a new epoch → readiness check → publication of the endpoint. The order is refined by the service protocol. By default the migrator is a pure bounded function; if it needs effects, they undergo separate authorization. Rollback of executable code does not undo published state or physical actions. Old code/schemas are retained until recovery obligations are fulfilled.

## 10. Nondeterminism, log and diagnostics

External clocks, randomness, the order of concurrent messages, service responses, admission decisions, cancellation, epoch changes and significant failures are within the nondeterminism boundary. The `replay` profile specifies exactly what is recorded, the log size, access and the conditions for replay. A single CodeID does not determine the behavior of a program with different inputs and bindings.

Replay uses the recorded results of effects; repeated commands to a real device or network are prohibited without a separate authorization mode. Gaps, an incompatible semantics version or missing retained data lead to an explicit refusal of full replay. Secrets and bearer tokens are not copied into the log; when inputs are redacted/concealed, the boundaries of possible replay are stated.

Diagnostics link the CodeID, source map, logical service, actor instance/epoch, operation ID and profile. The stack and logs are protected by separate rights. An operation failure contains a structured code and safe context; details of a closed resource are not disclosed to an unauthorized client. Metrics also account for messages not accepted due to quota, the cleanup queue and the cost of host calls.

## 11. ABI: Component Model and MIND host bindings

Marain compiles into a Wasm component and interface descriptions. WIT is not machine code. The chain of a system effect: **Wasm import → checking host adapter → protected IPC → service**. A direct `int 0x80`, native syscall or MMIO from a portable program does not exist; the platform transition resides in a narrow trusted adapter.

An isolated runtime does not receive standard WASI imports automatically. The profile fixes the supported versions of the Component Model, WIT, ABI, runtime and extensions; compatibility with an arbitrary toolchain version is not assumed. For the MVP an explicit submit/poll/cancel interface with resource tickets is used; a move to native async mechanisms is permitted after a separate review of the profile.

Example of a draft WIT interface showing the boundary between data and an opaque resource:

```wit
package mind:restricted-log@0.1.0;

interface logging {
    enum log-error {
        denied,
        revoked,
        quota-exceeded,
        too-large,
        unavailable,
    }

    resource log-sink {
        append: func(message: string) -> result<_, log-error>;
    }
}

world worker {
    import logging;
    use logging.{log-sink};
    export run: func(sink: log-sink);
}
```

Resource and own/borrow express the form of handle transfer: an owning handle in WIT text is written as the resource name (`log-sink`), a borrow as `borrow<log-sink>`. The MIND adapter additionally binds each handle to a valid capability. This interface has no constructor issuing an arbitrary sink: it is provided by the authorized launch. The maximum message length, budget and revocation semantics are set by a companion contract and checked by the adapter before unbounded allocation or decoding. [WIT: resources and worlds](https://component-model.bytecodealliance.org/design/wit.html).

Constraints already apply when converting the ABI representation into host values (lifting): lengths, total volume, depth, number of descriptors and CPU consumption are checked before the corresponding allocation/traversal. A check only inside the service method after an unbounded host string has been built is insufficient. If the chosen runtime does not allow this path to be bounded, the corresponding binding is not admitted into the profile.

Bindings are generated from the pinned contract together with validators and the hash of the behavioral part. ADTs are lowered into records/variants, concrete `Result`s into result; polymorphic exports are specialized; closures/continuations/ref do not cross the ABI. A large buffer is passed as a bounded sequence or as a MIND memory-resource with a separate ownership contract.

Versions of data, behavior, rights and resource constraints are checked together. Matching WIT types do not permit substituting a read service with a write service, increasing the permitted blocking time or changing the meaning of a successful response. An unsupported import is rejected at binding. There is no general `unsafe` escape in Core; FFI is packaged as a separate component with an explicit TCB contract.

## 12. Example: an actor with a log and a denied network

The example keeps the scenario of the original RFC, but makes a network refusal distinguishable and specifies explicit bindings. An empty successful response is not used to mask a missing right. Below is draft Marain pseudocode, not a test of a finished compiler.

```marain
effect Log {
    fn write(msg: String) -> Result<Unit, LogError>
}

effect Net {
    fn fetch(url: String) -> Result<Bytes, NetError>
}

fn agent() -> {Log, Net} Result<Unit, AgentError> {
    match perform Log.write("started") {
        Err(e) => return Err(AgentError.Log(e)),
        Ok(()) => (),
    }
    match perform Net.fetch("https://example.invalid/data") {
        Err(NetError.Denied) => {
            match perform Log.write("network denied by policy") {
                Ok(()) => Ok(()),
                Err(e) => Err(AgentError.Log(e)),
            }
        },
        Err(e) => Err(AgentError.Net(e)),
        Ok(data) => {
            // Content from the untrusted network does not get into the log automatically.
            drop(data)
            Ok(())
        },
    }
}

handler DenyNet for Net {
    fetch(_url, resume) => resume(Err(NetError.Denied))
}
```

Binding at the supervisor, shown separately from the code of the untrusted actor:

```text
given: a lifecycle scope, a spawn right, an allocatable child budget,
      an authorized narrow log-service endpoint and an approved agent image

1. Restrict the log endpoint: a set prefix, maximum message bytes,
   total quota/rate and session term. Inability to restrict means launch is refused.
2. Create a Log binding to this endpoint with a checking adapter.
3. Create a Net binding to the pure DenyNet, which has no network capability.
4. Check the image's requirements {Log, Net}, ABI, budget and authority to delegate.
5. spawn(scope, agent_image, init = Unit,
         bindings = {Log: restricted_log, Net: deny_net}, budget = child_budget).
```

The prefix is imposed by the restricted service/adapter, not by the untrusted actor itself. If the handler is called across a domain boundary, an endpoint with a protocol is passed; the parent's closure with the log-capability is not serialized. The append operation receives `Revoked` upon a subsequent revocation of the right. A successful launch does not change this outcome.

Verifiable variants of the example:

| Change | Expected result |
|---|---|
| Remove the Net binding | Static link-assembly error or loader refusal before the entrypoint |
| Keep DenyNet without a network cap | Launch is permitted; fetch returns Denied, there is no network action |
| Attach a real Net handler without the right | The binding or the operation is rejected; an import does not create a right |
| Revoke the log-capability after launch | A new write after revocation completes receives Revoked |
| Pass a string over the limit | TooLarge before unbounded copying/parsing |
| Exhaust the child budget | Declared refusal/suspension; the supervisor's reserve is preserved |
| Use the former `iso` after Accepted(MOVE) | Compiler error; a malicious bypass is also rejected by the execution boundary |

## 13. Manifest and launch authorization

Example of the form of a manifest; the numbers below illustrate constraints and do not prove the suitability of the runtime. Symbolic identifiers must be replaced with real CIDs at build time.

```yaml
manifest_version: 1
language_profile: marain-core-draft-0
entry_code: "<CodeID>"
component: "<ArtifactCID>"
build_recipe: "<BuildKey>"
contracts: ["<LogContractID>", "<NetContractID>"]
requires_effects: [Log, Net]
requested_limits:
  actors: 1
  heap_bytes: 8388608
  stack_bytes: 262144
  mailbox_messages: 32
  mailbox_bytes: 131072
  max_message_bytes: 4096
  capability_slots: 16
  cpu_budget_us: 2000
  cpu_period_us: 20000
persistence: none
```

The manifest requests resources and interfaces; it does not grant them. An authorized launch record separately links the image to the actually granted bindings, limits, epoch, supervisor, platform and runtime. Admission checks that the reserve is sufficient, taking into account overhead and recovery. Lower limits are permitted only if agreed by the component's contract; a reduction is not hidden from a program with a mandatory minimum.

## 14. Alternatives and accepted trade-offs

| Question | Proposed decision | Alternative and reason for restriction |
|---|---|---|
| New language or existing one | A small research Core; comparison on one vertical scenario | Rust/a restricted profile of a functional language + generated bindings may meet the goals more cheaply; the decision on wide adoption comes after the comparison |
| Effect and capability | Related but distinct levels | Identifying them does not describe handler refusal, revocation or domain scope |
| Handler model | Single-shot, no continuation escaping the scope in the MVP | Multi-shot complicates affine ownership and repetition of effects |
| Code identity | Canonical resolved typed Core and separate text | Raw ASTs depend on names/resolution; a binary hash is insufficient for source dependencies |
| Memory | ARC/reuse for acyclic Core; bounded cleanup | Tracing GC is possible in another non-critical profile, but requires explicit quotas and a pause model |
| Actor isolation | By trust zone, enforced by the runtime/OS | One process per actor is expensive; a shared environment without suitable protection is insufficient for mutually untrusted components |
| IPC | COPY first; managed memory objects as the next extension | Mandatory zero-copy is not justified for all sizes and platforms |
| ABI | Component Model + MIND behavior and rights | Plain WIT is insufficient; platform syscalls in bytecode destroy portability |
| Persistence | Explicit checkpoint contract | An automatic dump of all actors does not define external effects, rebind or portability |
| Safety plane | Separate `rt` qualification | Carrying Perceus/Wasm promises over to a physical control loop without timing analysis is unacceptable |

An existing high-level language is not considered incompatible with MIND CORE merely because it has an ordinary standard library. A specific profile is examined: whether ambient authority can be removed, the runtime bounded, bindings generated and effects accounted for. Marain must demonstrate a measurable advantage in the expressiveness of contracts, the quality of errors and the cost of maintenance.

## 15. Implementation plan and acceptance criteria

The prototype is developed on top of a host test bench with simulated services. Full DMA/MOVE and kernel guarantees are then verified on MIND CORE; host simulation does not attest them. The numbering `M0–M7` refers to the RFC, not to the earlier OS phases.

| Stage | Result | Acceptance condition |
|---|---|---|
| M0. Specification | Core grammar, type/effect/ownership rules, errors, model of handlers and local IPC | Contradictory/negative examples recorded; a clear trust boundary; comparison with the option without a new language |
| M1. Frontend and identity | Parser, resolution, type/effect checker, canonical codec, recursive groups and local CAS | CodeID test vectors: whitespace/local renaming are stable; changing a literal/effect/dependency changes the ID; round-trip preserves definitions |
| M2. Reference evaluator | Execution of Core, affine resources, single-shot handlers and the failure model | The full example of §12; double resume/use after MOVE prohibited; rights do not arise from the manifest |
| M3. Wasm component | Backend, chosen WIT/ABI version, runtime with limits, host bindings | Comparison with the evaluator on admissible programs; checks of the component ABI, unknown imports, corrupted handles and resource exhaustion |
| M4. Actors and protocols | Bounded mailbox, lifecycle scopes, epochs, supervision, tickets/cancel | The crash model before/after commit does not yield two owners; a payload is not lost without accounting; overflow, stale reply and supervisor failure are tested |
| M5. State and update | Head/retention/checkpoint/rebind, migration and reproducible AOT cache | Crash consistency, Head conflict, revocation unchanged after restore; changing target/runtime changes the recipe; unknown effects are not blindly repeated |
| M6. Cognitive plane pilot | One orchestration service and one bounded supervisor on MIND CORE | TCB, cost of messages, memory and bindings measured; fault injection and comparison with an alternative language passed; decision on continuation |
| M7. Separate extensions | `rt`, zero-copy, remote actors, advanced session types | Each extension gets a profile and an independent body of evidence; the absence of M7 is not hidden behind a successful M6 |

The model of resource and authorization boundaries is needed from M0. M3–M4 may proceed on host bindings after a minimal MIND IDL is pinned; porting uses stage II of the Constitution. M5 relies on the storage contract of stage IV, M6 on the required runtime/IPC services. M7 for physical control depends on qualification of the safety plane, not only on completion of the backend.

### 15.1. Mandatory evidence

| Area | Verifiable scenario or obligation |
|---|---|
| Types and effects | Preservation for the declared Core; progress with explicitly modeled waiting, errors and resource failures; negative cases of undeclared effects |
| Ownership | Use-after-move, a hidden cap inside val, an escaping borrow, double resume; the runtime does not trust the compiler alone |
| Authorization | Cap revoked between binding and call; forged handle; intermediary limit; manifest substitution grants no rights |
| IPC | Full queue, cancellation at the commit boundary, crash of both sides, old epochs, unexpected capabilities and parsing limits |
| Protocol | Invalid sequence, cyclic waiting, lost response, undetermined effect and the quota of reconcile records |
| Memory/CPU | ARC cascade, heap/stack exhaustion, infinite loop, hung host call, preservation of the recoverer's reserve |
| Storage | Power loss around commit/Head, checkpoint retention, publication conflict, incomplete dependencies |
| Restore/update | Snapshot with an old right, new SchemaID, unavailable rebind, conflict of exclusive owners, rollback after an external effect |
| Identity/AOT | Independent canonical test vectors; recursive groups; modified runtime; artifact substitution under a BuildKey |
| Replay | Recording of nondeterminism, gaps, unavailable data; absence of a repeated physical action |
| ABI boundary | Fuzzing of lift/lower and validators: huge lengths, nesting, invalid UTF-8, mismatch of resource kind and generation |

Proofs of types/protocols, tests, fuzzing and measurements are described separately. Protocols are also checked with a state model under failures; successful tests are not declared proof of the absence of all attacks. The applicability of each result is tied to the version of the compiler, runtime, ABI and platform.

## 16. Open questions with resolution conditions

| Question | Baseline decision of this RFC | When and by what it is closed |
|---|---|---|
| Exact textual spelling of constructs | Examples define semantics, not the final grammar | M0: grammar and diagnostic examples without ambiguity |
| Core codec, CID and hash algorithm | Versioned canonical encoding and migration are mandatory | M1: published test vectors and collision/migration analysis; until then CodeID is experimental |
| Backend/runtime and Component Model release | A specific version must be pinned; implicit WASI is prohibited | M3: a compatible minimal profile with measured constraints; a reference to "latest" is not permitted |
| Form of metering and quota values | Accounting for host work is mandatory; the example of §13 is not a platform norm | M3–M4: measurements, admission and fault tests; `rt` separately in M7 |
| Depth of session types | In the MVP — local state machines and runtime validation | M4/M7: composition analysis; progress promises are added only with a model |
| Cyclic values | Strong heap cycles prohibited, ID graphs available | After the MVP: a separate proposal for region/weak/cycle semantics and a cleanup budget |
| Checkpoint portability | Data schema + rebind; not a memory dump | M5: one fully verified schema transition and refusal on incompatibility |
| Cost of a new language | A research project until the pilot | M6: comparison of development/debugging, resources and TCB with an existing language and bindings |

A change to a baseline decision is issued as a new RFC revision stating its impact on CodeID, ABI, state, effects, trust and migration. Unresolved questions do not hinder modeling, but block the release of the corresponding stable format or the claim of a guarantee.

## 17. Conformance to the Constitution and changes relative to the original RFC

| Requirement of Constitution v1.4 | RFC sections |
|---|---|
| Isolation, memory transfer, data intake and progress: 2.1–2.13 | 2, 5, 7, 11 |
| Explicit rights, revocation, bootstrap and budget: 3.1–3.13 | 4, 5, 6, 13 |
| CID/Head, retention, provenance and migration: 4.1–4.13 | 8, 9 |
| Resource limits and time: 5.1–5.7 | 6, 7, 13, 15 |
| Lifecycle, effects, restore and epochs: 6.1–6.12 | 4, 7, 9, 10 |
| Partial connectivity and fencing: 7.1–7.7 | 7.3, 9, 15 |
| Cognitive/safety separation: 8.1–8.5 | 1, 6.2, 15 |
| Provenance, AOT and update: 9.1–9.9 | 8.3, 9, 13 |
| Information flows and observability: 10.1–10.7 | 2, 10, 12; the separate OS IFC policy is not replaced by effect types |
| Languages, effects, FFI and Babel: 11.1–11.11 | 2, 4, 11, 14 |
| Proof boundaries and migration: 12.1–12.9 | 1, 11, 15, 16 |

From the original RFC, the functional core, effects/handlers, addressable definitions, native actors, reference capabilities, the Perceus investigation and the target Wasm backend are retained. The following theses have been refined or replaced:

- "It is impossible to write code that violates invariants" is replaced by a list of properties, trusted boundaries and proof obligations.
- "An effect is equivalent to a mandate" is replaced by a three-level model: declaration → handler → valid authorization.
- "Hashes eliminate conflicts and provide a perfect free cache" is replaced by exact identity, retention, compatibility and a full BuildKey.
- "iso proves zero-copy without runtime checks" is replaced by distinguishing language ownership, byte transfer and hardware access.
- "ARC guarantees hard timing" is replaced by execution profiles and analysis of deallocation cascades, host work and the platform.
- "Empty data when the network is denied" is replaced by an explicit `Denied`, and the inherited environment by an explicit binding plan of the child actor.
- "Injecting int 0x80 into Wasm" is replaced by portable imports and a platform adapter outside Core.
- The roadmap is supplemented with a reference evaluator, negative examples, a crash/ownership model, restore/rebind, acceptance and conditions for resolving open questions.

**End of RFC 001, revision 0.2.**
