# 100003-APP-0148 — The hub's remaining parts: a user's export and deletion, a verdict's attachments, a name on the board by the user's leave, the proof of work raised by itself, the flag on a request for a credential

**Type:** host program (Linux), and the API · **Owner:** tools track (`APP`) · **Priority:** P2 · **Status:** open · **Blocked by:** — · **Main task:** [100003](100003-mind-hub-requests-conversation-check-and-updates.md), after stage 2 · **Roadmap:** track G · **Constitution:** MC-10.2, MC-10.3, MC-10.7, MC-11.5

## Problem

[100003-APP-0146](../issues-done/100003-APP-0146-the-separate-hub.done) built the separate hub. The design ([docs/hub/README.md](../docs/hub/README.md)) asks for more than it built ([docs/hub/server.md](../docs/hub/server.md), section 5):

- **Data rights (9):** a user exports their data, deletes a request that is not yet `accepted`, and deletes their account. An accepted task's issue stays in the repository, without their data.
- **A verdict's attachments (6.2):** a verdict comes "with text and attachments". The hub takes attachments on a request alone.
- **A name on the board (6.4):** a user may allow their name for a request. `hub-api 1`'s request form has no field for it, so the board names no one.
- **The proof of work (5.4):** the hub is to raise the difficulty when registrations come fast. It is the admin's setting alone.
- **A request that asks for a credential, a key or a change of authority (6.2)** is to be flagged to the admin. Nothing flags it; the agents read requests as data.

## Plan

- **Export:** `GET /hub/v1/me/export`, all of the user's data as JSON. **Deletion:** `DELETE /hub/v1/requests/{id}` before `accepted`, and `DELETE /hub/v1/me`. Each is an event, and the journal keeps that it happened without the deleted text: the event replaces the text with its SHA-256.
- **A verdict's attachments:** a message's attachment entries and `PUT requests/{id}/messages/{n}/attachments/{k}`, within the band's limits. This needs `hub-api` 2's message form: a new version of `improve/proto`'s form and vectors, with the old one refused by name (MC-12.4).
- **A name on the board:** `named` in the request's form, in the same new version.
- **The proof of work raised by itself:** the bits from the registrations of the last hour, within the admin's floor and `work::BITS_MAX`.
- **The flag:** words of credentials, keys and authority in a request's text mark it for the admin's page, and the agents' view says so.

## Acceptance criteria

- Host tests of each, in `tests/hub.rs`, and the conformance suite's checks of the routes.
- `improve/proto`'s vectors of the new forms, and the old version refused with its name.

## Related

[100003](100003-mind-hub-requests-conversation-check-and-updates.md), [100003-APP-0146](../issues-done/100003-APP-0146-the-separate-hub.done), [docs/hub/server.md](../docs/hub/server.md).
