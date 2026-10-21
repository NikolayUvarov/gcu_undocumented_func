# 750-KRN-0119 — Boot images and `init`'s grants for `effector` and `effector_gw`

**Type:** `init`, ABI (`common/abi.rs`: the boot images; the ABI version after 351-KRN-0115's), tests · **Owner:** kernel session · **Priority:** P2 · **Status:** open · **Blocked by:** the two services ([750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0007](750-EFF-0007-gateway-service.md)); [750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md) (started only in a mode); [750-KRN-0120](750-KRN-0120-writable-scopes-for-a-service.md), [750-KRN-0121](750-KRN-0121-a-lifecycle-client-for-named-services.md); the gateway's client of `updater` ([750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md)) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-3.4, MC-3.7, MC-7.5, MC-11.11

## Problem

The Effector agent is two boot services ([750](750-effector-agent.md)): `effector` speaks to the server and parses its input; `effector_gw` acts on the machine within the owner's policy. The list of boot images is part of the ABI, and each service's grants are an arm of `Init::start`, so both are the kernel track's. The agent track's request of 2026-10-09 is in [requests-KRN.md](requests-KRN.md), and the maintainer's handoff of 2026-10-15 ([requests-EFF.md](requests-EFF.md)) asks MIND Core to deliver the native agent.

## Plan

1. **Two boot images** in `BOOT_SERVICES` and `BOOT_FILES`, with the build, the signed manifest and the suites' service list. The ABI version goes up once, with whichever other kernel task changes the list at the same time.
2. **Both run only in the development or the maintenance mode** ([750-KRN-0143](750-KRN-0143-init-follows-the-development-and-maintenance-modes.md), the maintainer's decision of 2026-10-21, 750-EFF-0013). This replaces "off unless enabled in `data/services.txt`".
3. **`effector` gets, and nothing else:**
   - `rtc`;
   - `tls` (`SLOT_TLS`);
   - a `parse` client (`SLOT_PARSE`);
   - the `netpolicy` flow for program `effector`;
   - a read-only `vfs` client scoped to `data/effector/`;
   - its own private directory `system/effector` for the token, as `keystore` has its own (351-KRN-0040);
   - an endpoint to `effector_gw`.
4. **`effector_gw` gets, and nothing else:**
   - the lifecycle client of 750-KRN-0121;
   - the log read badge and `sysinfo`;
   - the writable scopes of 750-KRN-0120 on `data/effector/` and `data/apps/`;
   - read-only clients for `log:` and the file roots;
   - a client of `updater` ([750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md));
   - an `init` client badged `BADGE_REBOOT`;
   - its own private directory `system/effector_gw` for the command journal;
   - screen capture, for the policy's `screenshot`.

   No `tls`, no flow, no `parse` client, no device key. The test account's spawn rights are 501's.
5. **The ABI change** goes with `libmind`, `docs/api` and the `isolation` suite (AGENTS.md section 1).

## Acceptance criteria

- [ ] In QEMU, with both enabled, each service holds exactly the listed grants (the `isolation` suite's checks, x86 and aarch64).
- [ ] Without a mode, neither starts, and every other suite passes as before.

## Related

[750](750-effector-agent.md), [requests-EFF.md](requests-EFF.md), [750-EFF-0003](750-EFF-0003-interface-and-policy.md), [750-EFF-0006](750-EFF-0006-session-service.md), [750-EFF-0007](750-EFF-0007-gateway-service.md), [750-EFF-0008](750-EFF-0008-application-packages.md), [351-KRN-0116](351-KRN-0116-the-shells-client-of-the-updater.md).
