# Requests for the update track (UPD), not numbered yet

**Owner:** update track (TRACKS: the tracks module session; worked meanwhile by the storage session) · **Status:** open (1 request, 2026-10-21; a parser session that ends before its helper became [109-UPD-0022](../issues-done/109-UPD-0022-the-slots-channels-through-one-parser.done), 2026-10-21) · **Recorded by:** the kernel track (KRN)

The update track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The update track turns each into a task and removes it from this file, and the file goes when it is empty.

## An update through the link: the directory source (351-KRN-0141)

**Recorded by:** the kernel track (KRN), 2026-10-21, at the maintainer's request (an update of the kernel over the link), for [351-KRN-0141](351-KRN-0141-an-update-through-the-link.md).

### Problem

- **The plan.** On a machine reached only over the RS-485 link, the agent puts a release into `data/link/rel/` with `mindlink`, and the updater takes it from there: `update.txt` names `source data/link/rel`.
- **What the updater has.** `Source::Directory` (`updater/src/source.rs`) reads a directory on a mounted volume.
- **What is unknown.** No test runs it: the `update` suite uses the test HTTPS server.

### Plan (a proposal; the update track decides)

1. **The same checks.** Say whether the directory source checks everything the HTTPS source checks: the channel's signature, name and expiry, the manifest's digest and signature, each boot file's size and digest. Say also whether it resumes a cut file the same way. Fix what differs.
2. **A boot-volume path.** Confirm that a path on the boot volume (`data/link/rel`, with no volume prefix) is a valid source. `docs/update/updater.md` names only `usb:releases`.
3. **A run now.** Today the agent waits for the next automatic run (`every`). Two options:
   - `idl/update.wit` already lets a client ask `fetch` and `apply`. With the link's developer mode (800-KRN-0139, `dev` in `link.conf`), the kernel track can lend `link` the updater's client, as it lends the shell's (351-KRN-0116), if you agree.
   - Or the updater runs when its source changes.

   Say which you prefer.

### Acceptance criteria

The kernel track's `linkupdate` suite (351-KRN-0141) passes on x86 and aarch64: release 6 put through the link is fetched from the directory, booted on trial and confirmed, and a channel with a bad signature put first is refused.
