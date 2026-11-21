# 100003-APP-0151 — `improve` on MIND Core: the console

**Type:** program (MIND Core) · **Owner:** tools track (`APP`) · **Priority:** P1 · **Status:** open · **Blocked by:** `NET`, for a thread's long texts alone: `parse.json`'s strings longer than 512 bytes ([requests-NET.md](requests-NET.md#parsejson-strings-longer-than-512-bytes-for-100003-app-0151)). The rest does not wait · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), stage 4 · **Roadmap:** track G · **Constitution:** MC-10.2, MC-10.7, MC-11.5, MC-11.6, MC-11.9, MC-11.11, MC-12.1

## Problem

A MIND machine has no client of MIND Hub. The separate hub answers `hub-api 2`, and `improve` serves any other computer (100003-APP-0150); the design gives MIND Core the same commands ([docs/hub/README.md](../docs/hub/README.md), sections 2 and 9).

## Plan

- **A program `improve`,** a console program first, with the design's commands:
  - `register`, `status`, `answer`;
  - `new`, which shows what it sends and asks first;
  - `list`, `show N`, `reply N`, `check N`;
  - `devices`, `pair`, `revoke`.
- **Its authority** (design 9):
  - a flow grant to the hub's host on its port alone (MC-11.6);
  - TLS to the hub's pinned key, with no device certificate;
  - its own Ed25519 key as its own secret in `keystore` (`SLOT_SECRETS`);
  - its own directory (`SLOT_OWN`) for the hub's address and pins;
  - nothing else.
- **The hub's answers are parsed by `parse`** (MC-11.11). `improve` holds a long-term key and the network, so it reads the tree `parse` gives and checks it with `improve/proto`'s forms.
- **The hub:** `GET /hub/v1/challenge` gives the challenge in its body too. `mind::http` hands a program the typed head, not a header of the hub's own.
- **The check:** a QEMU suite against `tools/mindhub` on the host, at the address QEMU gives the host:
  - the hub runs over TLS with a certificate the suite makes;
  - `improve` registers, the admin approves through `mindhub`, `improve` files a request, and the agents' question is answered.

## Acceptance criteria

- **Host tests** of the program's own logic: the commands' words, the tree from `parse` read into the crate's forms, the signing.
- **The QEMU suite** above, on x86_64.
- **A thread's texts longer than 512 bytes** read once NET's change is in `main`; until then `improve` says it cannot read them, naming why.

## Related

[100003](100003-mind-hub-requests-conversation-check-and-updates.md), [100003-APP-0150](../issues-done/100003-APP-0150-improve-on-a-host.done) (on the tools branch until it lands), [docs/hub/README.md](../docs/hub/README.md), [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done), [000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done), [000-NET-0047](../issues-done/000-NET-0047-http-for-api-clients.done).
