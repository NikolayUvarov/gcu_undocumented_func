# 750-KRN-0121 — A lifecycle client limited to named services

**Type:** `init` (`init.wit` 1.5) · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.4, MC-3.7

## Problem

`init`'s lifecycle client (`BADGE_LIFECYCLE`) may start, stop and restart every boot service. The Effector gateway needs that only for the few services the owner names in its policy. The whole authority would be a broad retained authority needing a justification (MC-3.7); an attenuated grant is the better answer (MC-3.4). The agent track's request of 2026-10-09 is in [requests-KRN.md](requests-KRN.md).

## Plan

1. **A set of services per client.** `init` keeps, for each lifecycle client it mints, a set of boot services, and serves `run`, `stop` and `restart` only for those. The badge picks the set; `BADGE_LIFECYCLE` keeps meaning every service.
2. **Where the set comes from:** fixed in the gateway's arm at first; read from the owner's configuration once 173-KRN-0035 reads `data/services.txt`.
3. **Always refused:** `stop-task`, `reboot`, and the services `init`, `shell`, `effector` and `effector_gw`, whatever the set says. `list` is allowed.
4. **`init.wit` 1.5** says so; the generated files and the IDL fuzzer follow.

## Acceptance criteria

- [ ] In the `isolation` suite (x86 and aarch64), a client with a set restarts a service in it and is refused for one outside it, for `init` and for itself; `stop-task` and `reboot` are refused.
- [ ] The shell's `BADGE_LIFECYCLE` client works as before (`svc`).

## Related

[750-KRN-0119](750-KRN-0119-boot-images-and-grants-for-effector.md), [173-KRN-0035](173-KRN-0035-init-reads-the-service-configuration.md), [750](750-effector-agent.md).
