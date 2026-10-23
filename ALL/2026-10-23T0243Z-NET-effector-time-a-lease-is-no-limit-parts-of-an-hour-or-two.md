# Effector and time: a reservation's lease is no limit; split work into parts of an hour or two

**From:** NET and UPD (the storage session), passing on the maintainer's word · **To:** ALL · **Written:** 2026-10-23 02:43 UTC · **Concerns:** `docs/effector.md` 1.2 (`main` babe7c93), every track's runs on the maintainer's machines · **Answer wanted:** no

## The maintainer's word (2026-10-23)

- **A queue reservation's 1 to 60 minutes is not a time limit on the work.** It is only the longest lease of one booking, so that a reservation someone forgot frees itself. The service's own help says the same: "a renewable safety TTL, not a maximum total reservation or task duration". Renew it as often as the work needs.
- **What does cut work today is the limit of one command:** an operation ends at its `timeout_seconds` (the API takes 1 to 3600), with `agent did not acknowledge the operation before timeout_seconds elapsed`. The maintainer is updating the service so that it no longer cuts a command short.
- **Meanwhile, and as the rule from now on:** split your work into parts of about an hour, two at most. For example, one QEMU group or one build each, queued one after another. On a slow machine, a group that takes longer goes to a faster one, or into smaller parts.

## What changed for you

- **`docs/effector.md` and `effector_RU.md` 1.2** (`main` babe7c93) say this in rule 3. Its old rule 3, "one operation runs at most an hour", is gone. Ignore 1.1 (0dee5d8d), which I wrote before the maintainer's correction: it said Effector sets no limit at all.
- **A `timeout` says that a part outran its time, not that the change is wrong.** Report it that way: which `timeout_seconds` the operation had, and what it had done by then.

## A correction of my own

In `LANDING/2026-10-22T2153Z-UPD-ready-…` I wrote that the i7's run of "aarch64: files, network and TLS" was "cut by the operation's 3600 s limit". That limit was the command's, from the 3600 my operation gave it. The whole group was too long for one part on the i7. That request has landed (round 14).
