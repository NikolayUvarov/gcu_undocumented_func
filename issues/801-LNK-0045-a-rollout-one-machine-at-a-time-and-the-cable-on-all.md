# 801-LNK-0045 — A rollout one machine at a time, and the RS-485 line on all three

**Type:** tools (`scripts/`, `effector485`) · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress (2026-11-14) · **Blocked by:** S1 of [801](801-the-bench-stays-manageable.md) for the gate on the agent's return · **Main task:** [801](801-the-bench-stays-manageable.md) · **Constitution:** MC-9.3, MC-12.1

## Problem

On 2026-11-14 a release went to three machines one after another, and the third lost its network. Nothing stopped the rollout before it.

The RS-485 cable was moved by hand between the machines.

## Plan

1. **`scripts/rollout.py`:**
   - takes one release and a list of machines;
   - sends it to the first through the Effector server, or over the cable when the server cannot reach it;
   - waits for that machine's agent to come back on the new release;
   - only then goes on to the next.

   A machine that falls back, or whose agent does not come back within the bound, stops the rollout and shows its report (S2).
2. **Several machines on one line** ([800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md)) on the bench:
   - an address per machine in `link.conf`;
   - `effector485 --addresses`;
   - each machine named by its `agent`.

   The cable then stays where it is.

## Acceptance criteria

- The rollout stops at a machine whose release is rolled back, and names the reason.
- On the bench, the three machines are reached on one line by their addresses.

## Progress (2026-11-14)

**Plan item 1 is made:** `scripts/rollout.py`, described in [docs/link/rollout.md](../docs/link/rollout.md).

**What it takes:**
- one release: a signed build's `usb_root`, made into a release and its bundle as `link_update.py` and `release.py bundle` make them; or a bundle already made, checked first;
- the machines, as Effector agent IDs, in order.

**How it sends a release to each machine in turn:**
- **Through the server**, when hesere shows the machine `online` and `operations_ready`: it uploads the bundle once (`POST /api/v1/files`) and sends `update_agent` with `artifact_id`, `version`, `timeout_seconds` and `queue_ttl_seconds`.
- **Over the cable**, through `link_update.py`, when the server cannot reach the machine or its agent did not take the command (`denied`, `no-policy`, `expired`: nothing reached the updater). Only for the machine `--cable` names, and only after its log over the cable names that agent (`EFFECTOR: AGENT <id> TO`). `link_update.update` gained a `sent` call, so that the bound starts when the channel is put.

**When a machine counts as back:**
- `update_agent` answered `completed`, `updated N, slot S confirmed`, or, over the cable, the updater's `SLOT S CONFIRMED ON THE DISK`;
- the endpoint is taking operations again;
- this boot's ring (`collect_logs` of `ring`) names release N in `[UPDATER] SLOT S, VERSION N`, and `--commit` in `[INIT] BUILD`. For a late answer, whose text the server does not keep, this line is the proof.

**It stops the rollout,** prints the reason and the machine's log lines, and touches no later machine, when the machine:
- falls back (`fell-back`, `fell back: slot S runs version V, not N; the updater: …`);
- refuses the release, or does not confirm it;
- does not come back within `--wait` (1200 s);
- has a ring that names another version;
- has no path: unreachable on the server, and not on the cable.

**Its dry run** makes only GET requests to the server and reads only the cable's log.

**The API key** is read from its file, sent only in the `Authorization` header over HTTPS (or to loopback) with no redirect followed, and never printed.

**Tested on the host:** `tests/rollout_test.py`, in `scripts/host_tests.sh`. It runs against a stand-in server on 127.0.0.1, whose routes and fields are those of hesere's code (sst-test-deploy `server/operations.go`, `mindcore_update.go`, `file_transfer.go`, `endpoints.go`) and of a GET of the three bench endpoints on 2026-11-14. It shows:
- every machine updated in order, each after the one before answered, one by a late answer;
- a fallback stopping the rollout before the next machine, with the updater's lines;
- a machine that never comes back stopping it;
- the dry run sending nothing;
- an unknown machine or a wrong key sending nothing;
- the key never printed;
- a `denied` machine going over a scripted cable once its log names it;
- a cable reaching another machine getting nothing.

This is evidence on the host, not on the bench (MC-12.1).

**Left:**
- **The bench run (S8):** one release to the MacBook Pro, the DEXP and the Ryzen through hesere, with one built to lose the network stopping the rollout. It needs:
  - 801-KRN-0242 and EFF's `agent-step` call in the machines' release (S1), else a trial without the network is confirmed and only the bound stops the rollout;
  - `remote-update` on and `update apply` in each owner's policy (801-APP-0139, 0140);
  - the MacBook Pro's disk rewritten past 351-UPD-0026.
- **Plan item 2, the three machines on one line:** waits for the maintainer's wiring. Today `--cable` names the one machine the cable reaches, confirmed by its ring. That ring may have dropped the agent's start line, and then nothing is sent. Naming each machine by an `agent` line in `link.conf`, and passing `effector485 --agent link-<n>` through the adapter, come with the wiring.
- **The fallback's report in the heartbeat** (801-EFF-0029) is not read yet. The rollout reads the answer and the ring.

## Related

[801](801-the-bench-stays-manageable.md), [800-LNK-0026](800-LNK-0026-several-machines-on-one-line.md).
