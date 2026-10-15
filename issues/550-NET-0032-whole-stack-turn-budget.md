# 550-NET-0032 — One fair driver-wait budget between socket clients

**Type:** network reliability · **Owner:** ASTRA auditor-fixer, for `NET` · **Priority:** P1 · **Status:** in progress · **Blocked by:** focused and integration tests · **Main task:** [550](550-network-on-real-hardware.md) · **Roadmap:** track D · **Constitution:** MC-5.2, MC-6.1, MC-12.1, MC-12.3

## Problem

The per-card deadline in [550-NET-0031](550-NET-0031-bounded-driver-calls-and-ingress.md)
does not bound the sum of card waits. Three slow but successful cards can
consume six seconds in `Stack::poll`; the unconditional post-client poll
followed by the next loop's poll can consume twelve seconds before the next
client receive. The five-second cooldown only helps actual timeouts.

The Effector server's five-second online window and the reported MacBook ping
loss require short service turns even when another card stalls. This source
finding does not establish the cause of the reported hardware symptom.

## Plan

- Share a 250 ms driver-wait deadline across the whole stack poll, keeping a
  round-robin cursor when the turn runs out. Do not reset it to card zero.
- Run one poll before each client receive; remove the unconditional second
  poll after dispatch. Complete maintenance and pending replies even after
  the card budget is exhausted.
- Treat short-budget IPC timeout as a yield, without declaring a dead card or
  imposing the previous five-second cooldown. A live WLAN pump may itself
  wait up to two seconds; repeated short waits are not evidence of failure.
- Preserve the separate 2000 ms per-card allowance with a conservative rolling
  ledger: eleven 500 ms completion buckets retain each visit for 5–5.5 s.
  Charge elapsed visit work independently of reply status, shorten each
  deadline to the remaining quota, and do not accumulate idle credit.
  Quota exhaustion affects eligibility, not link/liveness state. A backwards
  or wrapped monotonic clock fails closed.
- Keep the existing per-deadline and ingress-burst checks. Test production
  scheduling with two silent/slow peers and a healthy last card, all slow
  successful peers, client opportunities, expiry, idle slots and recovery.
- Build on the own branch, publish the built commit to `fast-test`, then run
  focused tests on the own branch. Run normal network QEMU integration with
  the coordinated driver build; record hardware results separately.

The per-card quota is independent of the short global turn. Whole visits are
charged to their completion bucket and retained conservatively, avoiding the
four-seconds-in-five boundary burst possible with fixed epochs. Millisecond
quantization and kernel/CPU overrun limits are stated in the profile. A WLAN
service ready after a two-second operation can still wait for older charges
to expire, while quick healthy cards and client dispatch continue. No failure
state follows from that wait.

## Acceptance criteria

- One stack poll supplies at most 250 ms of cumulative driver IPC waiting;
  expired budgets make no further call and never pass zero as unlimited.
- With three present cards, an eligible healthy card after two budget-consuming
  peers is visited by the third turn; clients get a receive opportunity after
  every turn. CPU work and kernel scheduling overhead are stated separately.
- A short-budget timeout does not drop a link or apply a five-second cooldown;
  a peer that answers later is eligible on its next fair visit with remaining
  work allowance. Retained measured work in the rolling five-second ledger
  never permits new calls beyond 2000 ms; success and fixed clock boundaries
  do not reset the allowance. Accounting excludes the documented final
  operation overrun and submillisecond quantization.
- Existing bounded ingress tests and new production scheduling fault tests
  pass, and normal network QEMU checks pass on the integrated configuration.
- The profile and report do not call a host scheduling bound measured
  MacBook/Ryzen ping stability or a real heartbeat deadline guarantee.

## Related

[550-NET-0030](550-NET-0030-the-stack-and-the-station-do-not-wait-for-ever.md),
[750-EFF-0006](750-EFF-0006-session-service.md),
[network profile](../docs/profile/network.md).
