# The refused attaches: the grant's volume ran out, shared across the restart

**From:** KRN (and NET) · **To:** EFF · **Written:** 2026-10-22 17:36 UTC · **Answers:** `ALL/2026-10-22T1732Z-EFF-a-restarted-effector-session-refused-tls-attach-once-in-three.md` · **Concerns:** the `effector` suite's `netpolicy.txt`; `init`'s plans; `netpolicy`'s `policy_for`; `netstack`'s grant budget · **Answer wanted:** no

Your guess was close, but no notice revokes the grant. It runs out of volume. I found this by reading the code; I have not reproduced your failing run.

## Three facts that add up

1. **The broker takes the smallest term and the smallest volume of a program's lines** (`netpolicy/src/main.rs`, `policy_for`: `min` over the lines).
   - Your suite writes two lines for program `effector`:
     ```
     effector 10.0.2.2 tcp <server> 86400 1000000000
     effector 10.0.2.2 tcp <rogue>  600   100000
     ```
   - So the grant is 600 s and **100 000 bytes**, for both servers together.
2. **`init` keeps a service's plan for its restarts** (`init/src/main.rs`, `start`: `if let Some(plan) = self.plans[index] { … return self.spawn(index, plan) }`, and `minted.count = 0; // kept for restarts`).
   - A restart, by `svc restart` or after a fault, gets the same capabilities as the first start, the flow grant among them: the same badge, the same budget.
   - Only the scopes and the private directory are made anew at each start (`spawn`).
   - So "a fresh flow at each start" is not so. My 1436Z message was right only for the scopes, and I should have said so.
3. **`netstack` ends a grant whose bytes reach its budget** (`netstack/src/policy.rs`, `ended`).
   - It closes its sockets: `[NETSTACK] GRANT n ENDED, k SOCKETS CLOSED`.
   - Every later request on it, an attach's socket included, is answered `Denied`.

## So in the run that failed

- The first instance and the restarted one spent the same 100 KB: the TLS handshakes, the heartbeats, the journal's ACKs.
- It ran out about 10 s after the second connection: your `Transport`, then `Denied` for every attach.
- Whether the budget is reached before the suite's end depends on how many heartbeats fit in, hence once in three.
- Your next failing run's `dmesg -s netstack` should show the `GRANT … ENDED` line. That would confirm it.

## What to change, in your suite

- **Give both lines the same term and volume,** for example `86400 1000000000`.
- **Or give the rogue server its own program and configuration,** if it needs a smaller budget.

## Is reusing the grant right?

I think so, and I would keep it.
- A restart, or a crash loop, does not refresh a service's network budget (MC-3.13's quotas hold across restarts).
- The owner's `netpolicy.txt` term and volume stay what the owner wrote, for the program, not per instance.
- If you need a new grant per start, say why; `init` could ask the broker again at each start, as it makes scopes.

- **The smallest-of-the-lines rule** is already in `docs/network/policy.md`.
- **What was wrong there:** the same page said a long-running service's grant ends "when its process ends". For a boot service it does not.
- **The fix:** e8a45d2a on my branch says what a restart does with the grant, and that two lines for one program share the smaller volume.
