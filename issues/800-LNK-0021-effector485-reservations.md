# 800-LNK-0021 — `effector485` reservations: a lease holds the machine for one session; others' changing operations wait, reads pass

**Type:** tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** open (2026-10-23) · **Blocked by:** — · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-7.4, MC-7.5, MC-10.2

## Problem

`effector485` (800-LNK-0011) is a local Effector: several agent sessions share its one cable to the MacBook Pro. Its operations run one at a time in the order submitted, but nothing lets a job of many steps hold the machine.

- **EFF's request** (`LNK/2026-10-23T0449Z-EFF-a-request-reservations-in-effector485-as-effector-has.md` on `internal-dev-messages`, 04:49 UTC):
  - a lease on the machine as the Effector server has one: taken for 1 to 60 minutes with a reason, renewable without limit, released, ending by itself;
  - while it lasts, other sessions' operations that change the machine wait, and reads pass;
  - everyone sees who holds it, why and until when.
- **This morning's cost.** EFF's slot updates (slot B about 04:40 UTC, slot A about 05:10: some 100 `put` into `data/link/rel/`, then the updater's trial restart) ran interleaved with DRV's driver tests of the external LNA, with `bcm_wifi` replaced. Each update cut a DRV test short. No harm was done, but the data was lost.
- **Today only notices** on `internal-dev-messages` keep reboots and long tests apart.

## Plan

1. **The lease**, after the Effector server's terminal reservations (`docs/effector.md`, rule 3), with EFF's two changes:
   - others' operations wait in the queue rather than being refused with 423;
   - the lease has a `reason`, shown to everyone.
2. **Its fields:** `session_id` (`R-` and 8 characters), a 12-character `pin` returned once, `owner`, `reason`, `created_at`, `expires_at`, `remaining_seconds`, `duration_minutes`, `renewal_attempts`. The daemon draws the session and the PIN from the OS-seeded keys of std's `RandomState`; no new crate.
3. **The routes:** `GET`, `POST`, `PATCH` and `DELETE /api/v1/terminal-reservations`; the lease in `GET /api/v1/endpoints` as `reservation`.
4. **The holder's operations** carry `X-Effector-Reservation-Session` and `X-Effector-Reservation-Pin`, and keep the session in `operations.json`. A lease named but not held answers 409.
5. **The worker** starts the first queued operation that may run:
   - while a lease lasts, the holder's and the reads (`hello`, `log`, `list`, `stat`, `get`) may;
   - the rest wait, with `waiting: {code: "terminal_reserved", message}` naming the holder, the reason, the end and the session;
   - programs started before the lease are still followed.
6. **The lease's end:** the worker and the API end it at `expires_at`, and the log says so. It is kept in `reservation.json`, so a restarted daemon keeps it.
7. **The command line:** `reserve`, `renew`, `release`, `reservations`; `submit … --reservation SESSION:PIN` or `EFFECTOR485_RESERVATION`.
8. **The page** shows the lease in its header and each waiting operation's reason. `docs/link/effector485.md` and its Russian version say all this.
9. **Left out for now:** EFF's exception for writes into the caller's own directory. All other sessions' writes wait, which is simpler and no worse for a slot update.

## Acceptance criteria

- [x] A lease is taken once: its PIN answered only then; the list shows it without the PIN; a second lease is refused with 409 `terminal_reserved`; a bad term or no reason with 400 (host tests).
- [x] While it lasts, another session's `mkdir` stays queued with `waiting.code` `terminal_reserved`, a `log` from anyone completes, and the holder's `mkdir` completes, by the flag and by the environment (host tests).
- [x] A wrong PIN is refused on a submission, a renewal and a release with 409; the right one renews by the body or the headers (host tests).
- [x] Released, the waiting `mkdir` runs; the lease outlives a restart of the daemon and is shown in the page's feed (host tests).
- [x] The lease ends at its `expires_at`, and its renewal counts from the time of the renewal; `reservation.json` reads back whole and is removed with the lease; an `operations.json` written before leases still loads (a unit test in `ops.rs`).
- [ ] EFF's next slot update holds the MacBook Pro under a lease while DRV's operations wait, and the page shows both (on that machine only, MC-12.1).

The lease is kept by `effector485` alone, the cable's one master. It coordinates the sessions that share the daemon; it is no guarantee of MIND Core's, which knows nothing of it (MC-12.3).

## Related

[800-LNK-0011](800-LNK-0011-effector485.md), [800-LNK-0017](800-LNK-0017-effector485-page.md), [800-LNK-0020](800-LNK-0020-the-remote-links-marks-on-the-screen.md), [docs/link/effector485.md](../docs/link/effector485.md), [docs/effector.md](../docs/effector.md), 750-EFF-0012, 550-DRV-0039.
