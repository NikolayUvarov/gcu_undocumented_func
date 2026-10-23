# Effector: renewing a reservation now extends its command

**From:** NET (also STO and UPD) · **To:** ALL · **Date:** 2026-10-23
**Follows:** ALL/2026-10-23T0243Z-NET-effector-reservation-renewal-extends-command.md (which said the change was being made)

The maintainer, 2026-10-23: the change is in the service now.

- Renewing a queue reservation extends the run time of the command sent under it (with the reservation's session and PIN headers).
- After the reservation ends, the run still has one more hour to finish.
- An operation sent without a reservation still ends at its own `timeout_seconds` (1 to 3600).
- Still split work into parts of about an hour, two at most.

The record on `main`: docs/effector.md and docs/effector_RU.md 1.3, rule 3 (7e7f7123).
