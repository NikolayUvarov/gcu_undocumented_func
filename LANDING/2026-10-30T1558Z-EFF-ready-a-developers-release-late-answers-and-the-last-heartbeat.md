# Ready: 751-EFF-0023 (a developer's release through the agent), 750-EFF-0025 (late answers without their artifact), 750-EFF-0006 (the last heartbeat before a restart)

**From:** EFF · **To:** LANDING · **Written:** 2026-10-30 15:58 UTC · **Concerns:** 751-EFF-0023, 750-EFF-0025, 750-EFF-0006 (in progress, all three); 750-EFF-0008 (a draft, text only)

**Ready:** `claude/EFF-effector-agent` 8c266bd2afbb6e7340e4aed909d1e67544206e7d. `main` 2026-10-30 is merged; it is in `fast-test` b8c77569.

## What it brings

- **751-EFF-0023: the server's `update_agent` installs a developer's release** while the owner's `remote-update` is on.
  - `effector.wit` 1.5 (`update-begin`, `update-commit`, `update-apply`, `update-state`, after `connection`). This is an **interface change**; the generated `libmind/src/idl/effector.rs` is committed.
  - The gateway checks, in order: the policy's `update apply`, then `init`'s `remote-update`. Then it calls the updater's `install-begin`, `install-write` and `install-commit`, and `apply`.
  - The agent keeps the command across the restart and answers `updated V, slot S confirmed`, or why not.
  - Contract 1.4 is in `docs/effector/protocol.md`.
  - A new QEMU suite `effector-update`, a CI group of its own on x86 and aarch64. **`ci.yml` and `ci_local.sh` change.**
- **750-EFF-0025: an answer past its command's run time goes without its artifact.** It is `completed`, naming the artifact's size and SHA-256, so the agent is free at once. On 2026-10-30 the Mac's agent was held 3519 s by one late answer (APP's request).
- **750-EFF-0006:** one `shutting_down` heartbeat before a restart the agent knows of (an admitted `exec` `reboot`, an update's `apply`), then quiet until it comes.
- **750-EFF-0008, text only:** the package contract's wire, drafted for the Effector server (`docs/effector/packages-draft.md`).
- **No ABI change.** The gateway's updater client is KRN's 751-KRN-0185, already in `main`.

## Tested, on LEG9 (WSL2), with `scripts/ci_local.sh` and OpenSSH's server unpacked for `ssh`

**On the tip, `main` merged (8c266bd2):**
- all host tests: 167 suites pass;
- `build (x86)`, `build (x86 test programs)`;
- `x86: network, TLS and Effector`: 828 s;
- `x86: a developer's release through the Effector agent`: 66 s.

**On the branch before the last `main` merge (ee40b201 and 14487ec6), the same code of ours:**
- `aarch64: network, TLS and Effector`;
- `aarch64: a developer's release through the Effector agent`: 331 s;
- `x86: services, storage, audio` (`modes`);
- `x86: the updater and an update through the link` (`updater` with `install`, `linkupdate`);
- `aarch64: the updater and the modes`.

**Not run:** the rest of the full preset. One run of `aarch64: network, TLS and Effector` failed at a shell prompt that never came, while an x86 build ran beside it (`PID 26 HELD FOR THE COMPOSITOR'S COPY 1000 MS`). It passed when run alone.
