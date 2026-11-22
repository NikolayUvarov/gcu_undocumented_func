# Requests for the update track (UPD), not numbered yet

**Owner:** update track (the kernel session) · **Status:** open (1 waiting: a client that may only read the updater's state, APP's, 2026-11-21; STO's releases kept in the block store, 2026-11-08, became [351-UPD-0033](../issues-done/351-UPD-0033-the-updater-keeps-its-releases-in-the-store.done), made by STO) · **Recorded by:** the tools track (APP), 2026-10-28

The update track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The update track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

Taken (2026-10-30): the updater refusing every call that lends a buffer, for 351-APP-0029, went into [751-UPD-0032](751-UPD-0032-install-a-developers-release-from-the-gateway.md), whose install calls needed the same. The updater receives into slot 9 since 6a7ca80d on the kernel session's branch.


## A client of the updater that may only read its state, for 100003-APP-0151

**Asked by:** the tools track (APP), 2026-11-21 · **For:** [100003-APP-0151](100003-APP-0151-improve-on-mind-core.md), MIND Hub's `improve` on MIND Core ([docs/hub/README.md](../docs/hub/README.md), 6.3) · **Constitution:** MC-3.11, MC-11.5 · **With:** the kernel track's part, in [requests-KRN.md](requests-KRN.md#a-reading-client-of-the-updater-lent-to-a-program-for-100003-app-0151)

**Problem.** `improve` tells the hub the release the machine runs, so that a request a release carries waits for the user's check once the machine runs it.
- Today the user types the release (`improve version RELEASE`), read from `update status`.
- `improve` cannot ask the updater itself. The shell's client may `fetch`, `apply` and `rollback`, and lending it would give a hub's client the power to update the machine.

**Asked:**
- **A badge that reads alone**, say `BADGE_UPDATE_READ` in `mind::process`. A client with that badge may call `status`, `metadata` and `kept`, and gets `denied` for every other call, as `BADGE_UPDATE_GATEWAY` does for its own calls.
- It is a minor version of `update.wit` that states which calls that badge may make. No call changes.
- **The shell mints it** from its own client, if init gives that client the right to badge (the kernel track's part).

**What `improve` does with it:** at each start it reads `status`'s `running`, and tells the hub when that differs from the release it last told, kept in its own directory. `improve version` stays for a machine whose updater is not running.
