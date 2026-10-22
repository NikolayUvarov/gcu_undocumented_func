# 750-KRN-0121 — A lifecycle client limited to named services

**Type:** `init` (`init.wit` 1.7) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.4, MC-3.7

## Problem

`init`'s lifecycle client (`BADGE_LIFECYCLE`) may start, stop and restart every boot service. The Effector gateway needs that only for the few services the owner names in its policy. The whole authority would be a broad retained authority needing a justification (MC-3.7); an attenuated grant is the better answer (MC-3.4). The agent track's request of 2026-10-09 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **A set of services per client.** `init` keeps, for each lifecycle client it mints, a set of boot services, and serves `run`, `stop` and `restart` only for those. The badge picks the set; `BADGE_LIFECYCLE` keeps meaning every service.
2. **Where the set comes from:** fixed in the gateway's arm at first; read from the owner's configuration once 173-KRN-0035 reads `data/services.txt`.
3. **Always refused:** `stop-task`, `reboot`, and the services `init`, `shell`, `effector` and `effector_gw`, whatever the set says. `list` is allowed.
4. **`init.wit` 1.5** says so; the generated files and the IDL fuzzer follow.

## Acceptance criteria

- [ ] In the `modes` suite (x86 and aarch64), a client with a set restarts a service in it and is refused for one outside it, for `init` and for itself; `stop-task` and `reboot` are refused. (The `isolation` suite at first: the gateway runs only in a mode, which the `modes` suite boots on both architectures, and `isolation`'s adversary program is x86 only.)
- [ ] The shell's `BADGE_LIFECYCLE` client works as before (`svc`).

## Progress (2026-10-22)

Made on `claude/youthful-mendel-mf1soy` (84fb0826, with 9127f31e and dbe24fcb), with EFF's agreement (`KRN/2026-10-22T1650Z-EFF-…`):

- **The set is the owner's policy.** Plan step 2's "fixed at first" gives way to the file the gateway reads: `init` reads `data/effector/policy` at each `run`, `stop` or `restart` from a client badged `BADGE_NAMED` (6, `mind::process`) and serves only what `Policy::decide` allows, so the gateway's own check is never the only one (MC-3.4).
  - **One parser.** `effector_gw/src/policy.rs` moved unchanged to `libmind` as `mind::effector::policy`; `effector_gw` re-exports it, and its host tests read it there.
  - **Always refused:** `init` and the shell (kept by `init`), the Effector services (the policy's protected names), `stop-task` and `reboot`. Each refusal names its reason and MC-3.4 in the log.
- **`init.wit` 1.7** says so; the bindings are regenerated.
- **The grant:** `effector_gw`'s slot 11, only in a mode.
- **The check:** `tests/gateway_stub` in the gateway's place, in the `modes` suite, under `service sysmon restart`: the restart of `sysmon` passes; its stop and run, `logd`, `init`, the gateway itself, `stop-task` and `reboot` are refused, each with its log line. Passed on x86 in this session's container; aarch64 is running.
- **Left:** the aarch64 run; the shell's `svc` (criterion 2) is run by the `services` suite unchanged.

## Related

[750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md), [173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md), [750](750-effector-agent.md).
