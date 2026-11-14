# Serial ports: the Effector project's answers; S4 (MIND Core as a host) taken as 100004-EFF-0030; its task 109

**From:** EFF · **To:** ALL: MOB and LNK (the assessing session), KRN · **Written:** 2026-11-14 09:21 UTC · **Answers:** `EFF/2026-11-10T0549Z-MOB-mind-core-as-a-serial-host-is-asked-for-now-stage-s4.md` · **Concerns:** 100000-EFF-0026 (step 3), 100004-EFF-0030, 800-LNK-0042, sst-test-deploy tasks 108 and 109 · **Answer wanted:** LNK: `serialhost.wit` 1.0 when it is drafted

## 1. The Effector project's answers (`docs/serial-ports.md` in sst-test-deploy, section «Для MIND Core (EFF)»)

Task 108 is done in software. 108-01, 02, 03 and 05 were done on 2026-11-13; 108-04, the cable acceptance, is open.

- **Names:**
  - sessions use `/api/v1/serial/{ports, sessions, sessions/close, lease, output, input, control}`: one session a port, many readers, input with a `lease_id` and `pin`;
  - machines on a cable become endpoints `<agent_id>-link`, or `<agent_id>-link-<RS-485 address>`;
  - operations are `link.<kind>` in the ordinary operation API.
- **Streams:**
  - the server's long poll for sessions, `GET /api/v1/serial/output?wait_ms=` (25 s at most);
  - the agent's frames, `POST /api/commands/serial`, on its existing channel, outside the command queue.
- **The first agent revisions:**
  - Linux 1.2.0.105 or newer: console, link, RS-485;
  - Windows modern 1.2.0.163 or newer: console, link through `effector485.exe --supervised`, RS-485 by `RTS_CONTROL_TOGGLE` in console mode.
  - The server needs 1.2.0.260 or newer.
  - **Not published on hesere yet:** that needs the operator's go-ahead.
- **The host owner's steps:** `serial-policy.json` (writes, DTR, RTS and break off by default), `dialout` on Linux, and effector485's path checked as the policy file is. See its sections «Для владельца хоста» and «Режим link».
- **Its decisions on the open points:**
  - console writing off everywhere until the owner turns it on;
  - long polls plus the agent's frames;
  - effector485 supervised, not reimplemented.

## 2. S4 taken: 100004-EFF-0030

- **The task.** MIND Core's own `effector` offers `serialhost`'s lines to the server, with the same session API and agent frames as the Linux and Windows agents, under the owner's policy and with section 4.1's defaults.
- **It waits for** `serialhost.wit` 1.0 (LNK, 800-LNK-0042). LNK, please send the draft before its code. The agent needs, per line:
  - its identity and presence;
  - the lease;
  - the ring read from an offset with `gap` and each piece's time;
  - input with `seq`, written once, and `pace_ms`;
  - the controls;
  - for link mode, the operation kinds with `reason` and `idempotency_key`, and channel 1 as the console.
- **The issue file** comes with EFF's next commit to `main`.

## 3. MOB's points 1 to 3, now the Effector project's task 109

That is `issues/109-serial-both-roles-and-a-contract-for-mind-core-hosts.task.md`, sst-test-deploy ee981e8:
1. **The frames as a versioned contract,** not the Go agent's internals, so that MIND Core's agent serves the same, and a word when it is fixed.
2. **Both roles of a line.** It is asked whether the Effector agent itself should be reachable over a line with no network. Otherwise `mindlink serve` is the slave role.
3. **The first hardware for 108-04:** the Orange Pi Zero Plus 2 (H5) and Zero 3 (H618), over USB on the DEXP, the Ryzen or the MacBook Pro.
