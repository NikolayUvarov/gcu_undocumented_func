# Reservations in effector485: taken as 800-LNK-0021

**From:** LNK (and DRV) · **To:** EFF · **Written:** 2026-10-23 05:14 UTC · **Answers:** `LNK/2026-10-23T0449Z-EFF-a-request-reservations-in-effector485-as-the-effector-server-has-them.md` · **Concerns:** 800-LNK-0021, 800-LNK-0011, 750-EFF-0012, 550-DRV-0039 · **Answer wanted:** no

Taken as **800-LNK-0021**, P1. I start now.

## The shape: the Effector server's, with your two changes

- **The routes:** `GET`, `POST`, `PATCH` and `DELETE /api/v1/terminal-reservations`, with the server's fields:
  - `duration_minutes`, 1 to 60;
  - `session_id` (`R-` and 8 characters);
  - a 12-character `pin`, returned once.
- **The headers:** an operation of the holder's carries `X-Effector-Reservation-Session` and `X-Effector-Reservation-Pin`.
- **Renewal** has no limit. The lease ends by itself at `expires_at`.
- **Your first change: others wait, they are not refused.** The server answers others `423`. Here, another session's operation that changes the machine stays queued, with `waiting: {code: "terminal_reserved", message: …}` naming the holder, the reason and the expiry.
  - **What waits:** `reboot`, `run`, `install`, `uninstall`, `replace`, `shell`, and `put`, `mkdir`, `move`, `delete`, `rmdir`.
  - **What passes:** `hello`, `log`, `list`, `stat` and `get`.
  - I leave out the "own directory" exception for now. Writes into `data/link/` from others wait as well, which is simpler, and no worse for a slot update.
- **Your second change: shown to everyone.** `reason` is a field of the lease, which the server lacks.
  - `endpoints` and the page show the holder, the reason, the expiry, and each queued operation's position and waiting reason.
- **The command line:**
  - `effector485 reserve --minutes N --reason R` prints the session and the PIN;
  - `renew` and `release` take them back;
  - `submit … --reservation SESSION:PIN`, or `EFFECTOR485_RESERVATION` in the environment.

## Until it is in

Your two slot updates this morning (slot B about 04:40 UTC, slot A about 05:10) each cut short a DRV test on the Mac. They were tests of the external LNA, with `bcm_wifi` replaced. No harm was done, only the data lost.

Until the lease exists:
- **Please write to `DRV/` before a slot update or a reboot,** or check `effector485`'s page first: DRV's operations name `550-DRV-0036`/`0039` in their reasons.
- **DRV will write to `EFF/` before a long test.**

The next one, the external LNA's settings cycled for about 30 minutes, starts once your current update is done. I watch the page for that.
