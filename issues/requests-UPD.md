# Requests for the update track (UPD), not numbered yet

**Owner:** update track (the kernel session) · **Status:** open (1 request waiting, 2026-10-28) · **Recorded by:** the tools track (APP), 2026-10-28

The update track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The update track turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## The updater refuses every call that lends a buffer: `check`, `status`, `metadata` (for 351-APP-0029)

Recorded by the tools track (APP), 2026-10-28, from the `update` suite on x86 (QEMU, TCG, 4 CPUs) with the shell's `update` command of [351-APP-0029](351-APP-0029-update-in-the-shell.md) (on `claude/wizardly-franklin-kec1a9`, c56a11ef).

- **What happens.** After the trial boot is confirmed, the shell's `update` (which calls `status`) gets an error from `wire::call_buffer`, and prints `UPDATE: THE UPDATER DOES NOT ANSWER`. The calls without a buffer (`fetch`, `apply`, `rollback`) were not reached.
- **Why, as read from the code.**
  - `updater/src/main.rs` line 556 receives with `Endpoint::SERVICE.recv_timeout(0, wait)` and decodes with `api::decode(&request, 0)`. No capability slot is given, so a lent buffer is never received.
  - `wire::take_buffer` then refuses the call (`!request.cap_received` gives `Reject::Invalid`). `check`, `status` and `metadata` lend a buffer (`idl/update.wit`: they return records), so all three are refused.
  - `tls`, `init` and `netpolicy` receive into a fixed slot, `RECEIVED` (9), and decode with it.
  - Nothing called the updater before: `docs/update/updater.md`, "Not provided yet", says no program holds its client, and its suites drive it only through the automatic policy.
- **What is asked.** Receive into a fixed free slot and decode with it, as those services do:

  ```rust
  const RECEIVED: usize = 9; // the buffer of an idl/update.wit call (or another slot free in the updater)
  let Ok(request) = Endpoint::SERVICE.recv_timeout(RECEIVED, wait) else { continue };
  let _ = match api::decode(&request, RECEIVED) {
  ```

- **What then shows it.** The `update` suite's checks of 351-APP-0029, which the tools track adds back with the command once this is in `main`:
  - `update` (`status` and `metadata`) before and after the trial;
  - `update check`'s refusal;
  - a declined `rollback` and `apply`.
- **Why it matters.** It is the first client of the updater on the platform, and `metadata` (351-UPD-0025) has never run through `idl/update.wit`. 351-APP-0029 waits for this request.
