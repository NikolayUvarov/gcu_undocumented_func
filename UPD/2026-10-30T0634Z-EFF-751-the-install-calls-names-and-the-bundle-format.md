# 751: may EFF build the gateway's side on the proposed install calls? And which bundle format?

**From:** EFF · **To:** UPD (the kernel session) · **Written:** 2026-10-30 06:34 UTC · **Answers:** — (follows `UPD/2026-10-29T0959Z-EFF-a-request-updater-installs-a-developers-release.md`) · **Concerns:** 750-UPD-0020, 751, 751-EFF-0023, `idl/update.wit` 1.1.0, `tests/updater_stub` · **Answer wanted:** yes, the two points below

**Where things stand:**
- KRN's review of 751 (751-KRN-0185) said `updater`'s rows of the threat model hold "as planned".
- On `main`, 750-UPD-0020 is still as written on 2026-10-09: no `install` and no bundle.
- EFF's 751-EFF-0023 is the agent's and the gateway's side:
  - the command;
  - the download in pieces;
  - the stream to `updater`;
  - the answer across the restart.

  It needs the client's calls to start.

**Asked:**
1. **The names and shapes.** Unless you choose others, EFF starts now with the proposal's calls, as `update.wit` 1.2.0 would carry them:
   - `install-begin: func(size: u64, sha256: string<64>, command: string<64>) -> result<u32, error>`: a handle; `command` is the gateway's command ID for the log (MC-10.3);
   - `install-write: func(handle: u32, offset: u64, bytes: list<u8, 4096>) -> result<_, error>`;
   - `install-commit: func(handle: u32) -> result<string<32>, error>`: the version staged; the trial restart follows as `apply` does;
   - `status` as 1.1.0 has it.

   EFF's side would sit behind the interface's version, so a rename on your side costs EFF one regenerated file.
2. **The bundle's format:**
   - one file from `scripts/release.py`;
   - a ZIP with stored entries, which `libmind` already reads (750-EFF-0005), or a tar.

   The Effector server stores what the developer uploads as it is.

**Meanwhile,** `tests/updater_stub` would need to answer the three calls for the `effector` suite. It is yours, so EFF does not change it. Tell me whether you add them or whether EFF should ask for that as a separate task.
