# A request: `updater` installs a release the Effector gateway streams, only while the owner's `remote-update` is on (750-UPD-0020 extended, for 751)

**From:** EFF · **To:** UPD (the kernel session) · **Written:** 2026-10-29 09:59 UTC · **Concerns:** 750-UPD-0020, main task 751 (`main` 7b1d5e32), 751-EFF-0023 · **Answer wanted:** yes, the task number or 750-UPD-0020's new scope, and the bundle's format

**The maintainer (2026-10-29):** a developer agent must be able to update a machine's system through Effector, for tests, while the owner's permission is on. The permission is a word of `data/mode.conf`, `remote-update`, asked of KRN in `KRN/2026-10-29T0959Z-EFF-a-request-the-remote-update-permission-in-init.md`. [751](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/751-system-updates-at-a-developers-request.md) holds the design.

## Asked of `updater` (a proposal; UPD decides)

1. **The gateway's badge,** 750-UPD-0020's `BADGE_UPDATE_GATEWAY`. Its client may:
   - **`status`:** the state (idle, receiving, staged, trial, confirmed, fell back), the running, staged and last-known-good versions, and the last error;
   - **`install`, streamed** (new): `install-begin(size, sha256)`, `install-write(handle, offset, bytes)`, `install-commit(handle)`:
     - the gateway hands over a release bundle in pieces as the agent downloads it, so no file of it is written by the gateway or the agent;
     - `commit` checks the size and SHA-256, then the bundle as `updater` checks any source;
     - it writes the other slot and applies with a trial boot, exactly as the automatic apply does;
   - **`check` of the owner's channel,** as 750-UPD-0020 planned, for 750-EFF-0009.
2. **The permission is `updater`'s to check,** not the gateway's:
   - at `install-begin` and again at `install-commit`, `updater` asks `init`'s `mode()`;
   - it refuses with a reason unless `remote-update` is on.
3. **Nothing about trust changes.** The bundle's manifest and channel need the keys the machine already trusts. The version floor holds (MC-9.4), and the trial boot and fall-back stay (MC-9.3). The gateway's client can never change the channel, the keys or the floor (750-UPD-0020's own rule).
4. **The bundle:**
   - `scripts/release.py` writes a release as one file as well as the directory layout: for example `release.py bundle`, a tar or a ZIP with stored entries (`libmind` reads ZIP, 750-EFF-0005);
   - the developer uploads that one file to the Effector server.
5. **The log and the audit:** each install names the badge and the gateway's command ID, which the gateway passes, and the result (MC-10.3).

## Tests asked

- **The `update` suite:**
  - a client with the gateway's badge installs a test bundle with `remote-update` on;
  - it is refused with it off, before anything is written to the other slot;
  - a bundle with an untrusted key, or below the floor, is refused with it on.
- **`tests/updater_stub`** answers the new calls, so EFF can test the gateway's side in the `effector` suite meanwhile.

EFF's 751-EFF-0023 waits for this.
