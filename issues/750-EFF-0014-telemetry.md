# 750-EFF-0014 — Telemetry to the Effector server, as its other agents send it

**Type:** session, gateway, IDL · **Owner:** `EFF` · **Priority:** P1 · **Status:** open · **Blocked by:** — · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-10.2, MC-10.7, MC-12.1, MC-12.3

## Problem

- **What the maintainer saw (2026-10-22).** The MacBook's agent was online on `hesere.uz`, but its monitoring view was empty: "нет никакой телеметрии, как у других агентов".
- **What the others send.** The server's Linux and Windows agents publish a schema 1 sample every 30 s to `POST /api/telemetry` (Effector's `docs/monitoring.md`, `server/telemetry_store.go`). It holds processor load, memory, uptime, the commands answered, disks, network cards and components. The server counts each agent's traffic by `X-Effector-Agent-ID`.
- **What MIND lacked.** The session holds no local authority, so it could not count anything. The gateway had only `status`.

## Plan

1. **`effector.wit` 1.1** gets `sample`, the counters: the online processors' busy and idle time, uptime, the frame pool, the boot services with their PID, processor time, start time and memory, and the volumes' space. It is appended after the 1.0 calls, so their numbers stay. Like `status`, it is allowed to the session and the owner, changes nothing and carries no file, log or screen data.
2. **The gateway** answers it from `sysmon`, `init`'s list and its reading client of the volumes.
3. **The session** adds its own view of the network cards: `socket.wit` lets a flow grant ask `interfaces` and `cards`. It turns two samples into rates and sends one every 30 s (`effector/src/telemetry.rs`). A value MIND does not measure is left out, never sent as zero.
4. **Every request** names the agent in `X-Effector-Agent-ID`.

## Acceptance criteria

- [x] Host tests (`tests/effector_telemetry_host.rs`): the first sample has no rates; rates from two samples; no rate for a restarted process, another boot or a gap over an hour; 48 services fit; percentages at most 100.
- [x] The fixture checks a sample as the server's `validateTelemetry` does (`tests/effector_server.py`, its test).
- [x] The `effector` QEMU suite on x86_64: samples the fixture accepts, with processor load, the services with their processes, the boot volume and the guest's address.
- [ ] The same suite on aarch64.
- [ ] On the real server: the monitoring view of a MIND agent filled (750-EFF-0012).

## Related

[750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0007](750-EFF-0007-gateway-service.md), [750-EFF-0012](750-EFF-0012-run-against-the-effector-server.md), `docs/effector/protocol.md` 1.2, `docs/effector/README.md`.
