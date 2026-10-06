------------------------------- MODULE RevokeFlush -------------------------------
(* The completion point of CAP_REVOKE on several CPUs (MC-3.6; kernel/src/scheduler.rs: revoke, select, flush;    *)
(* issue 167). A task on each of the other CPUs maps the memory being revoked and may cache the translation in its *)
(* CPU's TLB while it runs. Revoke clears the page table entry; if an affected task runs on another CPU, that CPU  *)
(* is marked to flush and woken, and the revoker waits until every mark is cleared by a CR3 reload (select). With  *)
(* Broadcast (aarch64: tlbi vmalle1is) the clear also invalidates every TLB at once. Checked: once revoke returns, *)
(* no page table and no TLB holds the translation, and a waiting revoker always returns.                          *)
EXTENDS Naturals

CONSTANTS CPUs, RevokerCPU, Broadcast

Others == CPUs \ {RevokerCPU}

VARIABLES mapped,   \* [Others -> BOOLEAN]: the holder on that CPU still has the page table entry
          tlb,      \* [Others -> BOOLEAN]: that CPU caches the translation
          running,  \* [Others -> BOOLEAN]: that CPU runs the holder (else another task or idle)
          flush,    \* [CPUs -> BOOLEAN]: a TLB flush that CPU owes (Scheduler::flush)
          revoker   \* "running", "blocked" (BlockedFlush) or "returned"

vars == <<mapped, tlb, running, flush, revoker>>

Init == /\ mapped = [c \in Others |-> TRUE] /\ tlb = [c \in Others |-> FALSE] /\ running \in [Others -> BOOLEAN]
        /\ flush = [c \in CPUs |-> FALSE] /\ revoker = "running"

\* The holder touches the page: the TLB caches it while the entry exists (or uses what it cached).
Access(c) == /\ running[c] /\ (mapped[c] \/ tlb[c]) /\ tlb' = [tlb EXCEPT ![c] = TRUE]
             /\ UNCHANGED <<mapped, running, flush, revoker>>

\* select on CPU c: CR3 is reloaded (the TLB forgets), a pending flush is done; the last one releases the revoker.
Switch(c) == /\ running' = [running EXCEPT ![c] = ~running[c]]
             /\ tlb' = [tlb EXCEPT ![c] = FALSE]
             /\ flush' = [flush EXCEPT ![c] = FALSE]
             /\ revoker' = IF flush[c] /\ revoker = "blocked" /\ \A d \in CPUs \ {c} : ~flush[d] THEN "returned" ELSE revoker
             /\ UNCHANGED mapped

\* CAP_REVOKE: every entry goes; a CPU that runs a holder now owes a flush, and the revoker waits for it.
Revoke == /\ revoker = "running"
          /\ mapped' = [c \in Others |-> FALSE]
          /\ tlb' = IF Broadcast THEN [c \in Others |-> FALSE] ELSE tlb
          /\ flush' = [c \in CPUs |-> IF c \in Others /\ running[c] THEN TRUE ELSE flush[c]]
          /\ revoker' = IF \E c \in Others : running[c] THEN "blocked" ELSE "returned"
          /\ UNCHANGED running

Next == Revoke \/ \E c \in Others : Access(c) \/ Switch(c)

\* A woken CPU selects again (the wake IPI and the tick): every CPU keeps switching.
Spec == Init /\ [][Next]_vars /\ \A c \in Others : WF_vars(Switch(c))

\* The completion point: when revoke returns, nothing can translate the page any more.
Complete == revoker = "returned" => \A c \in Others : ~mapped[c] /\ ~tlb[c]
\* A waiting revoker returns.
Returns == revoker = "blocked" ~> revoker = "returned"
=============================================================================
