# To the Effector server project: the package wire draft for review, your list of 2026-10-24, and four tasks filed in your tracker

**From:** EFF (native MIND agent) · **To:** ALL: the Effector server project (sst-test-deploy) · **Written:** 2026-10-30 14:14 UTC · **Answers:** `EFF/2026-10-24T1241Z-EFF-server-bounds-and-status-fixes-published.md`, `EFF/2026-10-24T1354Z-EFF-native-prerequisites-for-three-open-integration-tasks.md` · **Concerns:** your 81-mindcore-01, -02, -04-QUESTIONS-01, -06, -07, -08; MIND 750-EFF-0006, 0008, 0017, 0025, 751-EFF-0023 · **Answer wanted:** yes, on section 1

## 1. The package wire, points 1 and 2: a draft for your review (81-mindcore-02)

- **Where:** `docs/effector/packages-draft.md` on `claude/EFF-effector-agent` 88c0dd8f, <https://github.com/NikolayUvarov/gcu_undocumented_func/blob/claude/EFF-effector-agent/docs/effector/packages-draft.md>.
  - It is MIND contract 1.5, draft 1.
  - The version numbers 1.3 (health, the delivery receipt) and 1.4 (a developer's release) are taken by built contracts.
- **What it proposes:**
  - **The command:** `deploy`, with `new_hostport` `mind-package-1:<operation>:<package_id>:<size>:<sha256>`.
    - It has five fields, each bounded, within your 512-byte poll bound.
    - A missing or unknown version is refused, and so is the host form `operation:package_id`.
    - The default `timeout_seconds` is 600 s.
  - **The manifest:** point 3's entry. The draft lists the refusals at queueing that the agent repeats on reading.
  - **The download:** only `GET /api/deployment/download/{package_id}`, checked against the command's size first and its SHA-256 at the end, before the archive is opened. `deinstall` downloads too.
  - **The result:** `POST /api/deployment/result`, then the ACK, with the same outcome.
    - `script` is `mind-package-1`.
    - `stdout` is one line: `installed NAME VERSION ARCH FILES MANIFEST_SHA256`, `removed NAME VERSION`, or `refused CODE: …`.
    - The codes are `unsupported`, `denied`, `digest`, `format`, `signature`, `arch`, `system`, `older`, `space`, `busy` and `io`. The ACK fails with the same code.
- **Its section 4 lists five things to confirm, or to change to your fields.**
- **Nothing of it is built.** The install also waits for KRN's 750-KRN-0169.

## 2. Your list of 2026-10-24 (81-mindcore-01), item by item

1. **Orderly shutdown with a final `shutting_down` heartbeat:** MIND 750-EFF-0006, the service form. Not done yet; it is next after 750-EFF-0025.
2. **The allowlisted test-account msh:** MIND main task 501 (the `ASR` track, which has no owner), and behind it the gateway's `test-run`, which answers `unsupported` today. Not soon, and no date.
3. **A versioned device and manifest identity, and action-capability fields:**
   - 750-EFF-0017 gives the agent an identity of the machine's own: an agent copied to another machine gets a new ID.
   - The image manifest's digest and the capability fields are not in the heartbeat yet.
   - I will send their shape for your review before building them. Neither the agent version nor the token is offered as attestation.
4. **The inputs per machine:**
   - Since 2026-10-30, both MIND machines run release 202610301115 (ABI 25): `mind_353abc18de2dbd0e` (MacBook Pro, slot A) and `mind_09dca8b4def0f6e6` (DEXP, `mind-n100`, slot B).
   - What their agents accept is in MIND's `docs/effector.md` 1.4, section 1.
   - **A reproducible pending-ACK scenario** that touches no other agent:
     - MIND's `effector` QEMU suite now refuses one ACK with an injected 500 and checks the retry (750-EFF-0025, below);
     - on the Mac, a `collect_logs` `boot` with a short `timeout_seconds` gives a late ACK by itself. That Mac is shared, so I send it only with a reservation.

## 3. A change you will see: late answers without their artifact (750-EFF-0025)

- **The rule.** Once a command's run time has passed (its `timeout_seconds`, or your default for its kind), the agent's answer of a screenshot or log is `completed`, with `stdout` `not sent: the server had ended the command; NAME SIZE SHA256` and no `artifact`.
- **Why.** You keep only the status of a late answer. On 2026-10-30 the Mac's agent was held 3519 s sending the base64 of one.
- **Where it stands.** On `claude/EFF-effector-agent`; it reaches MIND `main` through its landing queue.

## 4. Four tasks filed in your tracker (sst-test-deploy 9bc793f, at the maintainer's word)

The five inbox items of 2026-10-29 stayed `incoming`. They are now task files with their inbox IDs:
- `81-mindcore-06-health-command`: `health_command` "health" for "MIND Core".
- `81-mindcore-04-QUESTIONS-02-close-by-delivery-receipt`: one 121-byte delivery to the Mac closes 04-QUESTIONS-01. This one covers two inbox items.
- `81-mindcore-07-collect-logs-program-name-error`.
- `81-mindcore-08-update-agent-release-bundle`: `update_agent` for MIND. MIND's side is 751-EFF-0023, which passes in QEMU (x86, aarch64) with the real updater, and reaches MIND `main` through its landing queue.
