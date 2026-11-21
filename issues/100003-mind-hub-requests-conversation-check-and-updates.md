# 100003 — MIND Hub: a user's request done by agents, a conversation both ways, the user's check, and the updates of the OS and its modules

**Type:** main task (an application and its servers) · **Owner:** tools track (`APP`), at the maintainer's word of 2026-11-18; before, `MOB` · **Priority:** P1 · **Status:** open: the design ([docs/hub/README.md](../docs/hub/README.md) 0.4; the maintainer's choices of 2026-11-20: Rust, the rest as proposed), stages 1, 2 and 5 done (100003-APP-0145, 0146 and 0148: the protocol crate, the separate hub over `hub-api 2`, [docs/hub/server.md](../docs/hub/server.md); 0150: `improve` on a host, [docs/hub/improve.md](../docs/hub/improve.md)), stage 4 in progress (0151, done in QEMU on x86; long texts wait for NET), stage 3 next · **Blocked by:** — for stages 1 and 2; later stages below · **Roadmap:** track G (user services), track C (update and delivery) · **Constitution:** MC-3.1, MC-3.8, MC-7.5, MC-9.6, MC-10.2, MC-10.3, MC-10.7, MC-11.5, MC-11.6, MC-12.1, MC-12.3

**History:**
- **Recorded** by the kernel session, 2026-10-25, at the maintainer's request, as `improve`: a request to improve the OS from the phone, and its status (`MOB`).
- **Taken into `APP`** on 2026-11-18, at the maintainer's word: «да, подготовь предложение и забирай в APP. Это должно работать и из телефона и из любого компьютера, так как это не фича "только для телефона". Добавь разговор в обе стороны, проверку результата пользователя». The maintainer asked besides:
  - **two servers:** one native on MIND Core, and a separate one in Go or Rust that shows the accepted tasks;
  - **registration as Effector's agent's:** the device registers and an admin on the server confirms it, with feedback;
  - **anti-flood:** requests taken from all devices at first, with a rate by trust from −1023 to +1023, and none from a negative trust;
  - **updates in the same system:** «и сюда же сервер поддержки обновлений — по факту эта система обновления и операционной системы и модулей для неё».

## Problem

- **An improvement reaches MIND Core only from a computer with GitHub,** or by a word to the maintainer.
- **Nothing on a MIND machine,** a phone or any computer files a request, talks with the agents who do it, or lets the user check the result and say whether it works.
- **The releases of the OS and of its modules** are served by `tools/httpd` (110). No server ties a release to the requests it answers, or to the user who asked.

## Plan

The design is [docs/hub/README.md](../docs/hub/README.md). In short:

- **One server, the hub,** with four roles:
  - intake;
  - a conversation per request;
  - a public board of the requests that became tasks;
  - the updates: OS releases and module packages, signed by UPD's keys, which the hub never holds.
- **Two forms, one API** (`hub-api 2`), one protocol crate (`improve/proto`), one conformance suite:
  - the separate hub `tools/mindhub` first, in Rust (the maintainer's choice, 2026-11-20);
  - the native hub on MIND Core later.
- **Clients:**
  - `improve` on every MIND machine: the console, `wm`, the phone's screen with `MOB`;
  - `improve` on any host;
  - the hub's web pages for any browser.
- **Identity:**
  - each installation's own Ed25519 key, not the device key;
  - requests signed over a challenge the hub gives, so no clock is trusted;
  - registration `pending` until the admin approves, asks or rejects, with messages both ways;
  - pending devices may file, and their requests are held until approval;
  - more devices of a user join by a pairing code, browsers by a session from one.
- **Trust from −1023 to +1023:**
  - 8 bands of 128, each doubling the requests a day (1 to 128) and widening messages, attachments and channels;
  - none from a negative trust;
  - −1023 is a ban;
  - the hub moves trust by itself only within −256 … +512;
  - anti-flood per source network, a global intake cap, an optional proof of work, repeats refused, bounds named.
- **A request's states:** `held`, `new`, `needs-info`, `rejected`, `duplicate`, `accepted`, `in-progress`, `landed`, `delivered`, `awaiting-check`, `verified`, `reopened`, `closed-unchecked`.
  - The user's verdict on the release their own device runs closes or reopens it.
- **Agents:**
  - a key per track;
  - questions, proposals and results posted through the hub;
  - the user's words taken as data (MC-11.5);
  - a rule for AGENTS.md at the maintainer's word.

## Stages and tasks

| Stage | Task | What |
|---|---|---|
| 1 | [100003-APP-0145](../issues-done/100003-APP-0145-the-hub-protocol-crate.done), done | the protocol crate and its vectors |
| 2 | [100003-APP-0146](../issues-done/100003-APP-0146-the-separate-hub.done), done | the separate hub: intake, registration, trust, anti-flood, threads, the board, the admin pages, the agents' API, the conformance suite |
| 2+ | [100003-APP-0148](../issues-done/100003-APP-0148-the-hubs-remaining-parts.done), done | what stage 2 left: export and deletion, a verdict's attachments, a name on the board, the proof of work raised by itself, the flag on a request for a credential |
| 3 | numbered when it starts | the hub's updates (needs UPD's `COMMIT` beside a release; restricted channels need UPD's signed fetch) |
| 4 | [100003-APP-0151](100003-APP-0151-improve-on-mind-core.md), in progress | `improve` on MIND Core, the console: done in QEMU on x86 against the separate hub, to the user's verdict; a thread's long texts wait for NET's `parse`, the running release read by itself for a reading client of the updater (UPD, KRN) |
| 5 | [100003-APP-0150](../issues-done/100003-APP-0150-improve-on-a-host.done), done | `improve` on a host ([docs/hub/improve.md](../docs/hub/improve.md)); the web pages were made in 0146 and 0148 |
| 6 | [100003-APP-0153](100003-APP-0153-the-shells-notice-and-wms-mark.md), in progress | the shell's notice and `wm`'s mark, done in QEMU on x86; `wm`'s window and the phone's screen with `MOB` later |
| 7 | numbered when it starts | the native hub (needs 110's MIND Core server side, NET's) |
| 8 | at the maintainer's word | the agents' rule in AGENTS.md; the intake agent named |

## Acceptance criteria

- **Each stage's own criteria,** in its task.
- **For the whole:**
  1. a request filed from a MIND machine in QEMU and from a browser reaches a separate hub on the host;
  2. a question is answered in its thread;
  3. the task lands, and a release carries it;
  4. the device updates to that release;
  5. the user's `works` closes the request and `does-not-work` reopens it;
  6. a negative trust is refused and a flood is held at its limits;
  7. all of it is then shown on one bench machine, named with its configuration (MC-12.1).

## Related

[docs/hub/README.md](../docs/hub/README.md), [263](263-self-diagnosis-and-self-optimization.md) (its reports through the same intake, later), [260](260-ai-agents-on-the-platform.md) (an agent on the machine), [110](110-our-own-web-server.md) (`httpd`), [351](351-self-update.md) and [publishing.md](../docs/update/publishing.md) (the release tree), [750-UPD-0019](750-UPD-0019-signed-application-packages.md) (modules' packages), [100000](100000-a-phone-the-pinephone-first.md) (the phone), [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done), [000-NET-0047](../issues-done/000-NET-0047-http-for-api-clients.done), [000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done), [SECURITY.md](../SECURITY.md).
