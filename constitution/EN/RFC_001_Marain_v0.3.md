RFC 001 — Marain: the MIND CORE contract execution language
RFC revision: 0.3, final specification of the cognitive layer. Date: September 20, 2026. Status: integrated draft. Normative basis: MIND CORE Constitution v1.5.
Revision 0.3 (September 20, 2026, architecture finalization):
Support for SHARE_RW is excluded from the safe Core MVP subset and delegated to system adapters to prevent TOCTOU.
A system effect reconcile is introduced to return ownership (escrow) on network partitions (status OutcomeUnknown).
Validation of mandates in host bindings is tightened (check of epoch, owner and type on every ABI crossing).
1. Abstract and scope of the document
Marain is a draft of a functional, statically typed language with explicit effects, content-addressed definitions, isolated actors and message contracts. The execution target is the Wasm Component Model through restricted MIND bindings.
2. Execution model and distribution of guarantees
Execution path: Text → name resolution → typed Core IR → CodeID → checks → Wasm + WIT + MIND manifest → verification and linking → actor runtime / host bindings → protected handles and IPC → kernel mechanisms → services.
3. Minimal language core
3.1. Data, evaluation and errors: Strict evaluation order; lexical scopes; immutable values; primitives Unit, Bool, integers, Bytes, UTF-8 String, Option<T>, Result<T,E>, List<T>. 3.2. Signatures and purity: The effect set is inferred within a definition, but an exported interface MUST state it explicitly.
4. Effects, handlers and capabilities
4.1. Three distinct entities: Effect (a class of action), Handler (an interpretation), Capability (a permission in the running system). 4.2. Handler semantics: Deep handling with a one-shot continuation (resume). 4.3. Launch and revocation: Before an actor is launched, the binding plan MUST cover all mandatory effects of the entrypoint.
5. Ownership and resource types
Forms: iso<T>, val<T>, ref<'s,T>, Cap<I>, Endpoint<P,S>, MemoryObject<mode>. SHARE_RW is not part of the safe Core MVP. Its use is strictly excluded from the base profile and is delegated to trusted system adapters to prevent TOCTOU attacks. LEASE is revocable access to an object, not an ordinary language reference.
6. Resources, memory and time
6.1. ARC: Reference counting with liveness analysis and reuse (Perceus). Graphs of owning heap references are acyclic in the MVP. 6.2. Profiles: core, replay, rt. The Clock, Random, Spawn, Log and Store rights are granted explicitly.
7. Actors, channels and progress
7.1. Actor model: An actor has an entrypoint, a local heap, a bounded mailbox, a current epoch, a binding plan and an assigned supervisor. 7.2. Full channel contract: Includes states and transitions, sizes, ownership mode, ordering rules, deadline/cancellation, retries, failure and epoch change. 7.3. MOVE and the acceptance point: The transport has exactly two outcomes: Accepted or Rejected. 7.4. Protocol checking: The compiler checks that a local transition is permissible; the runtime checks the epoch, limits and state. 7.5. Unified operation outcome model: An external effect distinguishes NotStarted, Completed and OutcomeUnknown. To safely resolve the OutcomeUnknown status on network partitions, a system effect reconcile is introduced. It guarantees that if a timeout or partition is confirmed, the exclusive ownership (MOVE) placed in escrow will be correctly returned to the sender through a trusted gateway, preventing resource leaks or hangs.
8. Addressable code, dependencies and AOT
8.1. What is hashed: CodeID is computed over the canonical typed Core IR after name resolution. 8.2. Recursion and storage: Mutual recursion is represented by an explicit rec group. 8.3. Build and launch: The cache links a BuildKey to an ArtifactCID and verifiable provenance.
9. Storage, checkpoints and upgrade
A persistent actor declares a serializable state type, a SchemaID, a checkpoint point and a recovery contract. Capabilities are replaced by descriptions of needs for rebind. Upgrade follows a strict protocol of commit and epoch handover.
10. Nondeterminism, journal and diagnostics
External clocks, randomness and message order are written to the journal. The replay profile defines the conditions for reproduction. Secrets and bearer tokens are not copied into the journal.
11. ABI: Component Model and MIND host bindings
Marain compiles to a Wasm component. The chain of a system effect: Wasm import → verifying host adapter → protected IPC → service. Resource and own/borrow express the form in which handles are passed. The MIND adapter additionally binds each handle to a valid capability. Host bindings MUST validate the epoch, owner and type on every ABI crossing. Blind guessing or enumeration of numeric indices (handles) inside Wasm does not grant access to a kernel object, since the adapter strictly blocks unauthorized requests at the sandbox boundary.
12. Example: an actor with a journal and a forbidden network
(The example is retained as in specification 0.2: the agent correctly handles the network block and logs events through a narrow mandate.)
13. Manifest and launch authorization
The manifest requests resources and interfaces; it does not grant them. An authorized launch record binds the image to the bindings and limits actually granted.
14. Alternatives and accepted trade-offs
(See v0.2. The comparison with existing languages continues at stage M6.)
15. Implementation plan and acceptance criteria
Stages M0-M7. Required evidence covers: types, ownership, authorization, IPC, protocol, memory/CPU, storage, restore, identity/AOT, replay and the ABI boundary.
End of RFC 001, revision 0.3.

