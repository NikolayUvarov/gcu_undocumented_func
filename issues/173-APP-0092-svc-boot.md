# 173-APP-0092 — `svc boot`, `enable`, `disable`, `after`, `reset`: which services start at boot

**Type:** tools (`shell`) · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open (planned) · **Blocked by:** `KRN`: [173-KRN-0035](../issues-done/173-KRN-0035-init-reads-the-service-configuration.done) (`init` reads `data/services.txt` and reports the plan through `init.wit` `boot-plan`) · **Main task:** [173](173-boot-services-configuration.md) · **Roadmap:** track G · **Constitution:** MC-3.11, MC-12.3

**Recorded by:** the kernel track (KRN), 2026-10-09, in `requests-APP.md`, for main task 173 at the maintainer's request. Numbered by the tools track on 2026-10-20, at the maintainer's word.

## Problem

The maintainer wants a tool to turn boot services on and off and to order them. `init` will read `data/services.txt` and report the plan; nothing lets the user change the file but editing it by hand.

## Plan

- `svc boot`: the plan from `init` (order, enabled, essential, off and why) and how the last boot went.
- `svc enable <service>`, `svc disable <service>`, `svc after <service> <other>` edit `data/services.txt` through the shell's file client and say that the change applies at the next boot; `svc reset` removes the file.
- What `init` would refuse is refused here first: essential services, unknown names, an order against a dependency.
- The help and `docs/tools` (EN, RU).

## Acceptance criteria

The `tools` suite runs `svc disable tts`, reboots, and finds `tts` off in `svc boot`. `svc disable logd` is refused.

## Related

[173](173-boot-services-configuration.md), [173-KRN-0035](../issues-done/173-KRN-0035-init-reads-the-service-configuration.done).
