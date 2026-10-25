------------------------------ MODULE CapRevokeMove ------------------------------
(* The capability derivation tree of the MIND Core kernel (kernel/src/scheduler.rs: transfer, place, remove,        *)
(* revoke, mint, MEM_MAP; issue 167): capabilities in task slots, one send in flight per task, ghost nodes for       *)
(* dropped capabilities that still have descendants, and mappings made from a capability's node. Checked by TLC:  *)
(* a move-only object never has two writers (MC-2.6, MOVE), a revoked capability never comes back (MC-3.6), and   *)
(* rights only narrow along the tree (MC-3.5). Since 000-KRN-0104 revoke goes in steps, one node each, while the   *)
(* other tasks run: a copy, mint or move of anything under its root is refused until its last step, so the        *)
(* subtree never grows (MC-5.4), and once it ends nothing lives under a node it removed (MC-3.6).                  *)
EXTENDS Naturals, FiniteSets

CONSTANTS Tasks, Slots, MaxIds, RefuseMappedMove, RefuseCovered, Revokers
\* RefuseMappedMove: a memory capability its sender has mapped does not move (the kernel since issue 167; FALSE checks
\* the kernel before it, where TLC finds two writers). RefuseCovered: nothing under a revocation in progress is copied,
\* minted or moved (the kernel since 000-KRN-0104; FALSE lets the subtree grow while it is walked, and TLC finds it).
\* Revokers: the tasks that may revoke (all of them in the two-task configuration; one of three keeps the larger one small).

Objects == {"moveonly", "shared"}
Rights == {"r", "w", "g"}
\* The masks a mint or a copy applies: all rights, without grant, read only (each narrows differently).
Masks == {Rights, {"r", "w"}, {"r"}}
NULL == [none |-> TRUE]

\* A capability: its node (id, parent), the object, its rights.
Cap(i, p, o, r) == [id |-> i, parent |-> p, obj |-> o, rights |-> r]
MoveOnly(c) == "w" \in c.rights /\ "g" \notin c.rights

VARIABLES table,    \* [Tasks \X Slots -> capability or NULL]
          pending,  \* [Tasks -> NULL or [cap, from (a slot, or NULL for a copy), to]]: a send in flight
          ghosts,   \* nodes of removed capabilities that still have descendants
          maps,     \* mappings: [task, id, obj, writable]
          next,     \* the next fresh node id
          revoked,  \* ids removed by revoke (never to be seen again)
          revoking  \* [Tasks -> NULL or [root, gone, bound]]: a revocation in steps, the nodes it removed so far, and
                    \* the subtree as it began (a history variable, for NoGrowth)

vars == <<table, pending, ghosts, maps, next, revoked, revoking>>

Init ==
    \* One task holds a move-only object (writable, no grant), another a shared one it may copy.
    LET a == CHOOSE t \in Tasks : TRUE
        b == CHOOSE t \in Tasks : t # a
        s == CHOOSE s \in Slots : TRUE IN
    /\ table = [x \in Tasks \X Slots |-> IF x = <<a, s>> THEN Cap(1, 0, "moveonly", {"r", "w"})
                                         ELSE IF x = <<b, s>> THEN Cap(2, 0, "shared", {"r", "w", "g"}) ELSE NULL]
    /\ pending = [t \in Tasks |-> NULL]
    /\ ghosts = {}
    /\ maps = {}
    /\ next = 3
    /\ revoked = {}
    /\ revoking = [t \in Tasks |-> NULL]

Live == {table[x] : x \in {y \in Tasks \X Slots : table[y] # NULL}}
InFlight == {pending[t].cap : t \in {u \in Tasks : pending[u] # NULL}}
\* Copies in flight: a move in flight is its sender's slot until it arrives.
Copies == {pending[t].cap : t \in {u \in Tasks : pending[u] # NULL /\ pending[u].from = NULL}}

\* A task with a send in flight is blocked in it, and one revoking in its call: it makes no other call meanwhile.
Running(t) == pending[t] = NULL /\ revoking[t] = NULL

\* `ids` and their descendants through slots, copies in flight and ghosts (the kernel's index of the tree).
ClosureOf(ids) ==
    LET Step(s) == s \cup {c.id : c \in {d \in Live : d.parent \in s}} \cup {c.id : c \in {d \in Copies : d.parent \in s}}
                     \cup {g.id : g \in {h \in ghosts : h.parent \in s}}
        RECURSIVE Fix(_)
        Fix(s) == IF Step(s) = s THEN s ELSE Fix(Step(s)) IN
    Fix(ids)
\* The nodes whose children a revocation still removes: its root and the nodes it removed.
Frontier(t) == {revoking[t].root} \cup revoking[t].gone
\* Whether a revocation in progress covers node `id` (Scheduler::derive_refused).
Covered(id) == \E t \in Tasks : revoking[t] # NULL /\ id \in ClosureOf(Frontier(t))

\* Whether anything still derives from node `id` (Scheduler::derived).
Derived(id) == (\E g \in ghosts : g.parent = id) \/ (\E c \in Live : c.parent = id)
               \/ (\E c \in InFlight : c.parent = id) \/ (\E m \in maps : m.id = id)

\* Removes the capability in slot x (drop, overwrite): a ghost keeps its descendants revocable.
RemoveGhost(x) == IF table[x] # NULL /\ Derived(table[x].id) THEN ghosts \cup {[id |-> table[x].id, parent |-> table[x].parent]} ELSE ghosts

Mint(t, s, d, mask) ==
    /\ Running(t) /\ table[<<t, s>>] # NULL /\ table[<<t, d>>] = NULL /\ next <= MaxIds /\ ~(RefuseCovered /\ Covered(table[<<t, s>>].id))
    /\ LET c == table[<<t, s>>]
           allowed == IF "g" \in c.rights THEN c.rights ELSE c.rights \ {"w"} IN
       table' = [table EXCEPT ![<<t, d>>] = Cap(next, c.id, c.obj, allowed \cap mask)]
    /\ next' = next + 1
    /\ UNCHANGED <<pending, ghosts, maps, revoked, revoking>>

Send(t, s, to, move, mask) ==
    /\ Running(t) /\ table[<<t, s>>] # NULL /\ to # t /\ ~(RefuseCovered /\ Covered(table[<<t, s>>].id))
    /\ LET c == table[<<t, s>>] IN
       IF move THEN /\ ~(RefuseMappedMove /\ \E m \in maps : m.task = t /\ m.id = c.id)
                    /\ pending' = [pending EXCEPT ![t] = [cap |-> c, from |-> <<t, s>>, to |-> to]] /\ UNCHANGED next
       ELSE /\ ~MoveOnly(c) /\ next <= MaxIds
            /\ pending' = [pending EXCEPT ![t] = [cap |-> Cap(next, c.id, c.obj, c.rights \cap mask), from |-> NULL, to |-> to]]
            /\ next' = next + 1
    /\ UNCHANGED <<table, ghosts, maps, revoked, revoking>>

\* Scheduler::place: a move empties the sender's slot if it still holds the node, else the send is void.
Deliver(t, r) ==
    /\ pending[t] # NULL
    /\ LET p == pending[t] IN
       IF p.from # NULL /\ (table[p.from] = NULL \/ table[p.from].id # p.cap.id)
       THEN pending' = [pending EXCEPT ![t] = NULL] /\ UNCHANGED <<table, ghosts, maps>>
       ELSE LET cleared == IF p.from # NULL THEN [table EXCEPT ![p.from] = NULL] ELSE table IN
            /\ ghosts' = RemoveGhost(<<p.to, r>>)
            /\ table' = [cleared EXCEPT ![<<p.to, r>>] = p.cap]
            /\ pending' = [pending EXCEPT ![t] = NULL]
    /\ UNCHANGED <<maps, next, revoked, revoking>>

Drop(t, s) ==
    /\ Running(t) /\ table[<<t, s>>] # NULL
    /\ ghosts' = RemoveGhost(<<t, s>>)
    /\ table' = [table EXCEPT ![<<t, s>>] = NULL]
    /\ UNCHANGED <<pending, maps, next, revoked, revoking>>

Map(t, s) ==
    /\ Running(t) /\ table[<<t, s>>] # NULL /\ "r" \in table[<<t, s>>].rights
    /\ maps' = maps \cup {[task |-> t, id |-> table[<<t, s>>].id, obj |-> table[<<t, s>>].obj, writable |-> "w" \in table[<<t, s>>].rights]}
    /\ UNCHANGED <<table, pending, ghosts, next, revoked, revoking>>

Unmap(m) == m \in maps /\ Running(m.task) /\ maps' = maps \ {m} /\ UNCHANGED <<table, pending, ghosts, next, revoked, revoking>>

\* CAP_REVOKE (000-KRN-0104): the root is marked, and from then on nothing under it is copied, minted or moved.
BeginRevoke(t, s) ==
    /\ t \in Revokers /\ Running(t) /\ table[<<t, s>>] # NULL
    /\ LET root == table[<<t, s>>].id IN revoking' = [revoking EXCEPT ![t] = [root |-> root, gone |-> {}, bound |-> ClosureOf({root})]]
    /\ UNCHANGED <<table, pending, ghosts, maps, next, revoked>>

\* Node `id` removed by t's revocation: the mappings made from it go with it.
Removed(t, id) ==
    /\ maps' = {m \in maps : m.id # id}
    /\ revoked' = revoked \cup {id}
    /\ revoking' = [revoking EXCEPT ![t].gone = @ \cup {id}]

\* One node of the walk, between which every other task runs: a slot, a copy in flight or a ghost whose parent is the
\* root or a node the walk removed (a move in flight goes with its sender's slot: Deliver finds it gone).
RevokeStep(t) ==
    /\ revoking[t] # NULL
    /\ LET F == Frontier(t) IN
       \/ \E x \in Tasks \X Slots :
            /\ table[x] # NULL /\ table[x].parent \in F
            /\ table' = [table EXCEPT ![x] = NULL] /\ Removed(t, table[x].id) /\ UNCHANGED <<pending, ghosts>>
       \/ \E u \in Tasks :
            /\ pending[u] # NULL /\ pending[u].from = NULL /\ pending[u].cap.parent \in F
            /\ pending' = [pending EXCEPT ![u] = NULL] /\ Removed(t, pending[u].cap.id) /\ UNCHANGED <<table, ghosts>>
       \/ \E g \in ghosts :
            /\ g.parent \in F
            /\ ghosts' = ghosts \ {g} /\ Removed(t, g.id) /\ UNCHANGED <<table, pending>>
    /\ UNCHANGED next

\* The last step: nothing is left under the root, and the call returns (MC-3.6's completion point).
EndRevoke(t) ==
    /\ revoking[t] # NULL /\ ClosureOf(Frontier(t)) = Frontier(t)
    /\ revoking' = [revoking EXCEPT ![t] = NULL]
    /\ UNCHANGED <<table, pending, ghosts, maps, next, revoked>>

Exit(t) ==
    /\ revoking[t] = NULL /\ \E s \in Slots : table[<<t, s>>] # NULL
    /\ ghosts' = ghosts \cup {[id |-> table[<<t, s>>].id, parent |-> table[<<t, s>>].parent] : s \in {u \in Slots : table[<<t, u>>] # NULL /\ Derived(table[<<t, u>>].id)}}
    /\ table' = [x \in DOMAIN table |-> IF x[1] = t THEN NULL ELSE table[x]]
    /\ pending' = [pending EXCEPT ![t] = NULL]
    /\ maps' = {m \in maps : m.task # t}
    /\ UNCHANGED <<next, revoked, revoking>>

Next ==
    \/ \E t \in Tasks, s, d \in Slots, mask \in Masks : Mint(t, s, d, mask)
    \/ \E t, to \in Tasks, s \in Slots, move \in BOOLEAN, mask \in Masks : Send(t, s, to, move, mask)
    \/ \E t \in Tasks, r \in Slots : Deliver(t, r)
    \/ \E t \in Tasks, s \in Slots : Drop(t, s) \/ Map(t, s) \/ BeginRevoke(t, s)
    \/ \E m \in maps : Unmap(m)
    \/ \E t \in Tasks : Exit(t) \/ RevokeStep(t) \/ EndRevoke(t)

Spec == Init /\ [][Next]_vars

\* MOVE: a move-only object is written by one task at most (a writable capability or a writable mapping).
Writers(o) == {x[1] : x \in {y \in Tasks \X Slots : table[y] # NULL /\ table[y].obj = o /\ "w" \in table[y].rights}}
              \cup {m.task : m \in {n \in maps : n.obj = o /\ n.writable}}
OneWriter == Cardinality(Writers("moveonly")) <= 1

\* Revocation: a revoked node is in no slot, copy in flight or mapping again (a move of one in flight is void: its
\* sender's slot no longer holds it, and Deliver drops it).
NoRevival == /\ \A c \in Live \cup Copies : c.id \notin revoked
             /\ \A m \in maps : m.id \notin revoked

\* A revocation in progress removes and never gains: what is under its root stays within what was there as it began
\* (MC-5.4: the walk ends).
NoGrowth == \A t \in Tasks : revoking[t] # NULL => ClosureOf(Frontier(t)) \subseteq revoking[t].bound

\* Completion (MC-3.6): a child of a removed node lives only while the revocation that removed its parent goes on.
Completion == \A c \in Live \cup Copies \cup ghosts : c.parent \in revoked => \E t \in Tasks : revoking[t] # NULL /\ c.parent \in revoking[t].gone

\* Attenuation: a capability whose parent is live has no right its parent lacks.
Narrowing == \A c, p \in Live : c.parent = p.id => c.rights \subseteq p.rights

TypeOK == next \in 1..(MaxIds + 1)
=============================================================================
