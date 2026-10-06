------------------------------ MODULE CapRevokeMove ------------------------------
(* The capability derivation tree of the MIND Core kernel (kernel/src/scheduler.rs: transfer, place, remove,        *)
(* revoke, mint, MEM_MAP; issue 167): capabilities in task slots, one send in flight per task, ghost nodes for       *)
(* dropped capabilities that still have descendants, and mappings made from a capability's node. Checked by TLC:  *)
(* a move-only object never has two writers (MC-2.6, MOVE), a revoked capability never comes back (MC-3.6), and   *)
(* rights only narrow along the tree (MC-3.5).                                                                    *)
EXTENDS Naturals, FiniteSets

CONSTANTS Tasks, Slots, MaxIds, RefuseMappedMove
\* RefuseMappedMove: a memory capability its sender has mapped does not move (the kernel since issue 167; FALSE checks
\* the kernel before it, where TLC finds two writers).

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
          revoked   \* ids removed by revoke (never to be seen again)

vars == <<table, pending, ghosts, maps, next, revoked>>

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

Live == {table[x] : x \in {y \in Tasks \X Slots : table[y] # NULL}}
InFlight == {pending[t].cap : t \in {u \in Tasks : pending[u] # NULL}}

\* A task with a send in flight is blocked in it: it makes no other call until the send is delivered or cancelled.
Running(t) == pending[t] = NULL

\* Whether anything still derives from node `id` (Scheduler::derived).
Derived(id) == (\E g \in ghosts : g.parent = id) \/ (\E c \in Live : c.parent = id)
               \/ (\E c \in InFlight : c.parent = id) \/ (\E m \in maps : m.id = id)

\* Removes the capability in slot x (drop, overwrite): a ghost keeps its descendants revocable.
RemoveGhost(x) == IF table[x] # NULL /\ Derived(table[x].id) THEN ghosts \cup {[id |-> table[x].id, parent |-> table[x].parent]} ELSE ghosts

Mint(t, s, d, mask) ==
    /\ Running(t) /\ table[<<t, s>>] # NULL /\ table[<<t, d>>] = NULL /\ next <= MaxIds
    /\ LET c == table[<<t, s>>]
           allowed == IF "g" \in c.rights THEN c.rights ELSE c.rights \ {"w"} IN
       table' = [table EXCEPT ![<<t, d>>] = Cap(next, c.id, c.obj, allowed \cap mask)]
    /\ next' = next + 1
    /\ UNCHANGED <<pending, ghosts, maps, revoked>>

Send(t, s, to, move, mask) ==
    /\ pending[t] = NULL /\ table[<<t, s>>] # NULL /\ to # t
    /\ LET c == table[<<t, s>>] IN
       IF move THEN /\ ~(RefuseMappedMove /\ \E m \in maps : m.task = t /\ m.id = c.id)
                    /\ pending' = [pending EXCEPT ![t] = [cap |-> c, from |-> <<t, s>>, to |-> to]] /\ UNCHANGED next
       ELSE /\ ~MoveOnly(c) /\ next <= MaxIds
            /\ pending' = [pending EXCEPT ![t] = [cap |-> Cap(next, c.id, c.obj, c.rights \cap mask), from |-> NULL, to |-> to]]
            /\ next' = next + 1
    /\ UNCHANGED <<table, ghosts, maps, revoked>>

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
    /\ UNCHANGED <<maps, next, revoked>>

Drop(t, s) ==
    /\ Running(t) /\ table[<<t, s>>] # NULL
    /\ ghosts' = RemoveGhost(<<t, s>>)
    /\ table' = [table EXCEPT ![<<t, s>>] = NULL]
    /\ UNCHANGED <<pending, maps, next, revoked>>

Map(t, s) ==
    /\ Running(t) /\ table[<<t, s>>] # NULL /\ "r" \in table[<<t, s>>].rights
    /\ maps' = maps \cup {[task |-> t, id |-> table[<<t, s>>].id, obj |-> table[<<t, s>>].obj, writable |-> "w" \in table[<<t, s>>].rights]}
    /\ UNCHANGED <<table, pending, ghosts, next, revoked>>

Unmap(m) == m \in maps /\ Running(m.task) /\ maps' = maps \ {m} /\ UNCHANGED <<table, pending, ghosts, next, revoked>>

\* Scheduler::revoke: the closure of descendants of the root through slots, sends in flight and ghosts.
Closure(root) ==
    LET Step(ids) == ids \cup {c.id : c \in {d \in Live : d.parent \in ids}}
                         \cup {c.id : c \in {d \in InFlight : d.parent \in ids}}
                         \cup {g.id : g \in {h \in ghosts : h.parent \in ids}}
        RECURSIVE Fix(_)
        Fix(ids) == IF Step(ids) = ids THEN ids ELSE Fix(Step(ids)) IN
    Fix({root})

Revoke(t, s) ==
    /\ Running(t) /\ table[<<t, s>>] # NULL
    /\ LET root == table[<<t, s>>].id
           ids == Closure(root)
           gone == ids \ {root} IN
       /\ table' = [x \in DOMAIN table |-> IF table[x] # NULL /\ table[x].parent \in ids /\ table[x].id # root THEN NULL ELSE table[x]]
       /\ pending' = [u \in Tasks |-> IF pending[u] # NULL /\ (pending[u].cap.parent \in ids \/ (pending[u].from # NULL /\ pending[u].cap.id \in gone)) THEN NULL ELSE pending[u]]
       /\ ghosts' = {g \in ghosts : g.parent \notin ids}
       /\ maps' = {m \in maps : m.id \notin gone}
       /\ revoked' = revoked \cup gone
    /\ UNCHANGED next

Exit(t) ==
    /\ \E s \in Slots : table[<<t, s>>] # NULL
    /\ ghosts' = ghosts \cup {[id |-> table[<<t, s>>].id, parent |-> table[<<t, s>>].parent] : s \in {u \in Slots : table[<<t, u>>] # NULL /\ Derived(table[<<t, u>>].id)}}
    /\ table' = [x \in DOMAIN table |-> IF x[1] = t THEN NULL ELSE table[x]]
    /\ pending' = [pending EXCEPT ![t] = NULL]
    /\ maps' = {m \in maps : m.task # t}
    /\ UNCHANGED <<next, revoked>>

Next ==
    \/ \E t \in Tasks, s, d \in Slots, mask \in Masks : Mint(t, s, d, mask)
    \/ \E t, to \in Tasks, s \in Slots, move \in BOOLEAN, mask \in Masks : Send(t, s, to, move, mask)
    \/ \E t \in Tasks, r \in Slots : Deliver(t, r)
    \/ \E t \in Tasks, s \in Slots : Drop(t, s) \/ Map(t, s) \/ Revoke(t, s)
    \/ \E m \in maps : Unmap(m)
    \/ \E t \in Tasks : Exit(t)

Spec == Init /\ [][Next]_vars

\* MOVE: a move-only object is written by one task at most (a writable capability or a writable mapping).
Writers(o) == {x[1] : x \in {y \in Tasks \X Slots : table[y] # NULL /\ table[y].obj = o /\ "w" \in table[y].rights}}
              \cup {m.task : m \in {n \in maps : n.obj = o /\ n.writable}}
OneWriter == Cardinality(Writers("moveonly")) <= 1

\* Revocation: a revoked node is in no slot, send or mapping again.
NoRevival == /\ \A c \in Live \cup InFlight : c.id \notin revoked
             /\ \A m \in maps : m.id \notin revoked

\* Attenuation: a capability whose parent is live has no right its parent lacks.
Narrowing == \A c, p \in Live : c.parent = p.id => c.rights \subseteq p.rights

TypeOK == next \in 1..(MaxIds + 1)
=============================================================================
