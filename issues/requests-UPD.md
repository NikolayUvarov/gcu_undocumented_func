# Requests for the update and provenance track (UPD), not numbered yet

**Owner:** update and provenance track · **Status:** open (the updater's checks in a parser of their own became [109-UPD-0017](109-UPD-0017-the-updaters-checks-in-their-own-parser.md), 2026-10-14; the Effector gateway's client of `updater` and signed application packages became [750-UPD-0020](750-UPD-0020-the-gateways-client-of-the-updater.md) and [750-UPD-0019](750-UPD-0019-signed-application-packages.md), 2026-10-20) · **Recorded by:** the Effector agent track (EFF), 2026-10-09

The update and provenance track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The track turns each into a task and removes it from this file, and the file goes when it is empty.

## A note: `init` says how the boot went (000-KRN-0102)

**Recorded by:** the kernel track (KRN), 2026-10-14, for [000-KRN-0102](000-KRN-0102-init-says-how-the-boot-went.md), the update track's request for 351-UPD-0007.

**What changed:** `init.wit` 1.4 `boot()` answers the slot the machine booted from, whether it was on trial, and whether `init` confirmed it. `init` serves requests only once it has confirmed or refused the boot, so a trial boot answered `confirmed: false` was refused. Any client of `init` may ask, the updater's badged one too.

**Asked:** the updater may ask for it every second while it waits for the confirmation, and keep the kernel's deadline as the bound. The `update` check then writes the confirmed record within seconds of `[INIT] TRIAL BOOT CONFIRMED` (the acceptance criterion of the request).
