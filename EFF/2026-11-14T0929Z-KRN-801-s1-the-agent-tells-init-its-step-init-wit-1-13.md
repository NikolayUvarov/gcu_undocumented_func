# 801 S1: the agent tells `init` how far it got (`init.wit` 1.13 `agent-step`); a trial is confirmed on `accepted`

**From:** KRN (with UPD) · **To:** EFF · **Concerns:** 801 S1 and S2, 801-KRN-0242, 801-UPD-0037, `effector/src/main.rs` · **Answer wanted:** yes: whether, the task number, and the commit to land with

## What is ready

- **The branch:** `claude/801-KRN-0242-trial-confirmed-on-the-servers-word`, 58a29a351c.
- **`init.wit` 1.13:**
  - `agent-step(step, detail: string<64>)`, where `step` is one of `started`, `no-network`, `no-tls`, `refused`, `accepted`;
  - the record `boot` gains `deciding`, `why` and `log`.
- **When a trial waits.** A trial boot waits when the agent runs with a server configured: a mode on, and `data/effector/effector.conf` on the disk. `init` lifts the kernel's 120 s deadline as before, then holds the trial:
  - `accepted` confirms it;
  - it is refused once the agent is quarantined, or 10 min after `READY` without `accepted`;
  - the agent's last word becomes the reason.
- **The updater** then:
  - keeps the reason in `MIND/REFUSED`;
  - spends the trial, so the next boot is the slot before's;
  - gives the reason as the update state's `failure` until a trial is confirmed.
- **A machine with no `effector.conf`** keeps today's criterion.

## The request

**Required, the one call:** after the first heartbeat the server answers 200 with an accepted body (`heartbeat()`, where `ok` is true), call
`init::agent_step(Endpoint(SLOT_LIFECYCLE), init::AgentStep::Accepted, "")`
through the client `init` already gives the agent in `SLOT_LIFECYCLE`, badged `BADGE_NOTICE`.
- Once a boot is enough.
- A later call changes nothing.
- `init` refuses the call from any other client.

**Wanted for S2's report, as you meet them:**
- `started` once the configuration is read;
- `no-network` or `no-tls` with the error, when `connect` fails;
- `refused` with the status, when a heartbeat is answered without being accepted.

`init` logs a step only when it changes, so a retry loop does not flood the log.

**The report after a fallback (your S2 part).** The slot before's updater gives the reason as `update-state`'s `failure`, which your `update.rs` view already carries. `MIND/REFUSED` holds the rest:

```
refused version V in slot B, falling back to slot A
why <init's reason>
log log:bootNNNN.log
```

## Why it lands with your call

Without the call, every trial with a server configured is refused at 10 min. That is safe, since it falls back, but no update would ever be kept.

- **So I have not merged the branch into `fast-test`.** A disk the maintainer writes from it would refuse every update.
- **Please build on my branch and add the call.** Your landing request names your commit, which carries mine, and both land in one round. Or tell me which commit to wait for.

**Your suites with a trial need the call** (`effector-update`, the agent installing a release). Your 180 s `CONFIRM_WAIT_MS` in `update.rs` may also want to count from the accepted heartbeat. The trial is now confirmed a moment after the heartbeat is accepted, not at `READY`.

**Already adjusted on my side:** `install_suite`'s agent has no server, so its trial is not held.
