# 801-EFF-0029 — The fallback's report, sent on the first connection after it

**Type:** session · **Owner:** `EFF` · **Priority:** P1 · **Status:** in progress (2026-11-14) · **Blocked by:** — (801-UPD-0037's record, in 801-KRN-0242's branch) · **Main task:** [801](801-the-bench-stays-manageable.md) (S2) · **Constitution:** MC-9.3, MC-9.9, MC-12.3

## Problem

- **The decision.** After a trial that could not reach the server, the machine restarts into the slot that had the connection (801's S1, UPD's). The maintainer wants the reasons reported.
- **What the agent has to report on its first connection afterwards:**
  - the refused release's version;
  - the step that failed: no radio, no link, no address, no route, no TLS, the server refused, or the agent did not start;
  - the trial boot's log lines that show it.

## Plan

1. **Read UPD's reason record** in the slot that had the connection, once UPD names its place and format. The trial's boot log stays on `log:`.
2. **The heartbeat's `update` field.** The server keeps it already for its other agents (`UpdateStatus`: `state`, `build_number`, `sha256`, `result`, `updated_at`). The agent sends:
   - `state` `rolled_back`;
   - the refused version as `build_number`;
   - the failed step and a short reason as `result`.

   This is also 751-EFF-0023's plan item 6. The field names are the server's, checked against its `server/storage.go`.
3. **The log lines.** The trial's boot log is `log:`'s boot before the current one. An operator collects it with `collect_logs`, and the report names that log.
4. **Once only:**
   - the report is kept in `system/effector` until a heartbeat carrying it is accepted, then marked sent;
   - the next update clears it.
5. **Tests:** the `effector-update` QEMU suite with a release built to lose the network. It is rolled back by UPD's S1, and the agent's first heartbeat carries `rolled_back`, the version and the step.

## Acceptance criteria

- [ ] The `effector-update` suite, x86 and aarch64, as above.
- [ ] On a bench machine (801's S8): a release built to lose the network, rolled back, and its report seen on hesere.

## Related

[801](801-the-bench-stays-manageable.md), [751-EFF-0023](751-EFF-0023-the-agent-installs-a-developers-release.md).

## Progress (2026-11-14)

- **UPD's record (801-UPD-0037, on 801-KRN-0242's branch).**
  - The updater of the slot before keeps the reason in `system/updater/refused`.
  - It gives `update-state`'s `failure` as `version V in slot S, falling back to slot T: WHY` until a trial is confirmed.
  - The trial's boot log is `log:bootNNNN.log`.
- **The agent.**
  - It reads `update-state` at its start and with each telemetry sample.
  - While `failure` is such a line, every heartbeat carries `update: {"state": "rolled_back", "build_number": V, "result": the line}`, the server's own `UpdateStatus`, so the first connection after the fallback reports it.
  - It is logged once: `A FALLBACK, REPORTED TO THE SERVER`.
  - Once the updater confirms a trial, the line is gone, and so is the field.
  - No file of the agent's own is needed: the updater keeps the record.
- **Host test:** `a_fallback_is_reported_in_the_heartbeats_update` (`tests/effector_wire_host.rs`).
- **The QEMU path** needs a trial refused after 801-KRN-0242's 10 min. It is asked of 801-ASR-0014's `manageable` suite, with S1. Not run yet.
