# Requests for the update and provenance track (UPD), not numbered yet

**Owner:** update and provenance track · **Status:** open · **Recorded by:** the Effector agent track (EFF), 2026-10-09

The update and provenance track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The track turns each into a task and removes it from this file, and the file goes when it is empty.

## A client of `updater` for the Effector gateway (700)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [700](700-effector-agent.md) at the maintainer's request.

### Problem

The Effector server asks an agent to update itself. On MIND Core that can only mean a system update through `updater` (351): the agent is part of the signed image. The gateway `effector_gw` needs to ask for it, and it must not be able to change what is installed by any other path (MC-8.5, MC-7.5).

### Plan (a proposal; the update track decides)

- `idl/update.wit` (351-UPD-0007) has a client badge for the gateway that may call `check`, `fetch`, `status` and, if `init`'s arm allows it, `apply`.
- The badge cannot change the channel, the trusted keys or the minimum version. Those stay with the owner's configuration and the shell's `update` command.
- `status` gives the versions of the running, staged, trial and last-known-good releases, and the last error, in a form the gateway can report.

### Acceptance criteria

In the `updater` suite, a client with the gateway's badge checks, fetches and stages a test release. It is refused when it tries to change the channel. Without `apply` in its arm it cannot activate the release.

## Signed application packages: format, key role, verification (700)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [700](700-effector-agent.md) at the maintainer's request.

### Problem

The Effector server installs software by sending a package. MIND Core signs its system image, but it has no format for an application installed at run time, and no key role that may sign one. The loader does not verify applications (profile: "Not met"). The gateway (700-EFF-0008) must not install code on the strength of TLS alone (MC-9.2, MC-9.8).

### Plan (a proposal; the update track decides)

- **The package manifest:**
  - the format of `MANIFEST` (file, SHA-256, `.mind_request`) for one application, with name and version;
  - an Ed25519 signature, as `MANIFEST.SIG`.
- **The key role.** "Application package" as a key role in 351-UPD-0009:
  - separate from the release key;
  - which keys the owner trusts is a line of the gateway's policy.
- **Tooling and checks:**
  - a host tool that builds and signs a package (beside `scripts/release.py`);
  - a verification routine shared by the gateway and the loader.
- **The loader's part.** The loader checks an installed application against its recorded manifest before running it. That part is a kernel task (requests-KRN, when this format exists).

### Acceptance criteria

- The host tool signs a test package, and the routine accepts it.
- The routine refuses a package with a changed file, a wrong key or a lower version than one already installed, in host tests.
