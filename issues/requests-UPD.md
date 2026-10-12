# Requests for the update and provenance track (UPD), not numbered yet

**Owner:** update and provenance track · **Status:** open · **Recorded by:** the Effector agent track (EFF), 2026-10-09

The update and provenance track numbers its own tasks (`NNN-UPD-MMMM`), so requests from other tracks wait here. The track turns each into a task and removes it from this file, and the file goes when it is empty.

## A client of `updater` for the Effector gateway (750)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [750](750-effector-agent.md) at the maintainer's request.

### Problem

The Effector server asks an agent to update itself. On MIND Core that can only mean a system update through `updater` (351): the agent is part of the signed image. The gateway `effector_gw` needs to ask for it, and it must not be able to change what is installed by any other path (MC-8.5, MC-7.5).

### Plan (a proposal; the update track decides)

- `idl/update.wit` 1.0 (351-UPD-0007, done) gets a client badge for the gateway that may call `check`, `fetch` and `status`, and `apply` and `rollback` only if `init`'s arm allows them; `init`'s part is a kernel task.
- The badge cannot change the channel, the trusted keys or the minimum version. Those stay with the owner's configuration and the shell's `update` command.
- `status` gives the versions of the running, staged, trial and last-known-good releases, and the last error, in a form the gateway can report.

### Acceptance criteria

In the `updater` suite, a client with the gateway's badge checks, fetches and stages a test release. It is refused when it tries to change the channel. Without `apply` in its arm it cannot activate the release.

## Signed application packages: format, key role, verification (750)

**Recorded by:** the Effector agent track (EFF), 2026-10-09, for main task [750](750-effector-agent.md) at the maintainer's request.

### Problem

The Effector server installs software by sending a package. MIND Core signs its system image, but it has no format for an application installed at run time, and no key role that may sign one. The loader does not verify applications (profile: "Not met"). The gateway (750-EFF-0008) must not install code on the strength of TLS alone (MC-9.2, MC-9.8).

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

## The updater's checks in a parser of their own (109-NET-0010)

**Recorded by:** the kernel track (KRN), 2026-10-12, for [109-NET-0010](109-NET-0010-a-parser-per-session.md), done by the kernel session at the maintainer's word.

### Problem

Each `download` now parses its heads in a `parse` process of its own, which holds only that session's endpoint and ends with it. The updater's channel, manifests and file heads still go to the shared `parse` service, so one check's input reaches the process that parses the next.

### Plan (a proposal; the update track decides)

- **In `updater/src/source.rs` and `updater/src/main.rs`,** a check or a fetch takes a `mind::parse::Session::start(loader, Endpoint(SLOT_PARSE))` and passes its `endpoint()` where `SLOT_PARSE` goes now. The session ends when it is dropped.
- **What the updater needs from `init`,** which the kernel track adds when asked: a client of `loader` badged so that it may only start helpers (`loader.wit` 1.8 `helper`, [109-KRN-0096](109-KRN-0096-the-loader-starts-a-helper.md)), and an endpoint quota of 1. It gets no right to start programs.

### Acceptance criteria

The `update` suite's check runs its parsing in a helper that holds only its endpoint (`stat caps`), and the helper is gone when the check ends.
