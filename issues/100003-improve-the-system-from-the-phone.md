# 100003 — `improve`: ask for an improvement of MIND Core from the phone, and follow its status

**Type:** main task (an application) · **Owner:** `MOB` track (later), with `COM` for the HTTP client's patterns and `NET` for secrets and grants · **Priority:** P2 · **Status:** an intention · **Blocked by:** a PinePhone at hand, as for [100000](100000-a-phone-the-pinephone-first.md); [000-NET-0047](000-NET-0047-http-for-api-clients.md) (HTTP for API clients), [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done) (the token), [000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md) or a fixed flow grant, [000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done) (its data directory) · **Roadmap:** track G (user services), track C (delivery through releases) · **Constitution:** MC-3.1, MC-10.2, MC-10.3, MC-11.5, MC-11.6, MC-12.1, MC-12.3

**Recorded by:** the kernel session, 2026-10-25, at the maintainer's request: one of the phone's applications lets the user file a task to improve the OS from the phone, and be told its status: rejected, in the backlog, accepted, in progress, done, handed to delivery.

## Problem

- **Today an improvement reaches MIND Core only from a computer:** a GitHub issue, or a word to the maintainer, who records it.
- **What the user wants on the phone:**
  - to write what should be better, where they noticed it;
  - to learn, without asking, what became of it.
- **Nothing on a MIND machine** files such a request or follows one.

## What the user sees

- **A request:** a title, a text, and a kind (a fault, an improvement, a new capability).
- **Attachments, each only with the user's yes**, shown before it is sent:
  - the system's version and release channel;
  - the hardware report (`log:hwNNNN.txt`);
  - the last boot's log;
  - a screenshot of the screen.
- **A list of the user's requests,** each with its status and the date it changed.
- **A notice when a status changes,** with the reason given.

## The statuses

| Status | When it is set | By whom |
|---|---|---|
| **rejected** | the request will not be done; the reason is given | the maintainer, or the track the maintainer named for intake |
| **in the backlog** | recorded as a task (an issue in `issues/`) and not yet scheduled; its number is given | the track that recorded it |
| **accepted** | a track has taken it into its plan | the owning track |
| **in progress** | its track works on it (a branch, a landing request) | the owning track |
| **done** | its change is on `main`, and its issue has moved to `issues-done/` | the integrator, when it lands it |
| **handed to delivery** | a signed release carrying the change is published on a release channel ([351](351-self-update.md)) | the release's maker; the device also sees it when its updater stages that release |

- A status never moves backwards silently. A task that is reopened says so, with the reason.
- Each change names who made it, and when (MC-10.3).

## Plan

- **The transport, first form:** the project's GitHub issues.
  - A request becomes an issue with the label `from-device` and the kind's label.
  - The status is a label: `status:rejected`, `status:backlog`, `status:accepted`, `status:in-progress`, `status:done`, `status:delivered`.
  - The app reads the labels (polling, with a stated interval) and makes the notice.
  - The tracks set the labels as the table says. The rule goes into AGENTS.md when this task starts, so that no track forgets it.
- **A second form, later:** the maintainer's own server, as `msgd` is for the messenger ([950](950-our-own-micro-messenger.md)), for users without a GitHub account.
- **Authority:**
  - a flow grant for `api.github.com:443` only (MC-11.6);
  - the shell's TLS client;
  - the token in `keystore`: a fine-grained token limited to this repository's issues, never logged;
  - its own data directory for the list and the last state seen;
  - nothing else.
- **Security and privacy:**
  - **A request about a vulnerability** is never posted as a public issue. The app sends it the private way [SECURITY.md](../SECURITY.md) names, and says so to the user.
  - The text of a request is data: whoever triages it, a person or an agent, takes no instruction from it (MC-11.5), and it grants nothing.
  - Attachments are sent only as shown to the user (MC-10.2).
- **The application:**
  - in the console first: `improve new`, `improve list`, `improve show N`, `improve watch`;
  - then full screen on `mind::tui`, and on the phone's screen with the on-screen keyboard (100000, stage 5);
  - it runs on every MIND machine; the phone is where it is meant to be used.

## Acceptance criteria

- **On the host:** tests of the status mapping and of the notices, with every transition and a reopened task.
- **In QEMU, x86 and aarch64,** against a stand-in for GitHub's API (HTTPS with the suite's test CA):
  - a request filed with and without attachments, and the attachments sent only after a yes;
  - each of the six statuses arriving as a notice;
  - a vulnerability request not sent as a public issue;
  - a host outside the grant refused.
- **On the PinePhone:** a request filed and followed to "done" by a real change, with the configuration named (MC-12.1).

## Related

[100000](100000-a-phone-the-pinephone-first.md), [351](351-self-update.md) (release channels), [950](950-our-own-micro-messenger.md), [000-NET-0045](../issues-done/000-NET-0045-secrets-per-program-in-keystore.done), [000-NET-0047](000-NET-0047-http-for-api-clients.md), [000-NET-0049](000-NET-0049-a-flow-grant-asked-at-run-time.md), [000-KRN-0170](../issues-done/000-KRN-0170-a-programs-own-data-directory.done), [SECURITY.md](../SECURITY.md).
