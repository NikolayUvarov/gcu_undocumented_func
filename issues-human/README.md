# issues-human/ — tasks only a person can do

Things the agents working on this repository cannot do: repository settings on GitHub (agents have no admin access), legal decisions, and coordination between agent sessions. Mark an item done by ticking it and noting the date; delete the file section when it no longer matters.

## 1. Enable private vulnerability reporting

`SECURITY.md` tells reporters to use **Security → Report a vulnerability**; that button exists only when the feature is on.

- [ ] Repository → **Settings** → **Security** (Code security) → **Private vulnerability reporting** → **Enable**.

## 2. Repository description and topics

Makes the project findable on GitHub.

- [ ] Repository main page → gear icon next to **About**:
  - Description: `MIND Core: capability microkernel in Rust for x86-64 UEFI, with ring-3 drivers and services`
  - Topics: `rust`, `microkernel`, `operating-system`, `osdev`, `uefi`, `x86-64`, `capabilities`
  - Keep "Releases" and "Packages" unticked until there is a release.

The repository keeps its name (`gcu_undocumented_func`); the `repository` field of every `Cargo.toml` already points to it.

## 3. Optional: protect `main`

CI (`.github/workflows/ci.yml`) runs on every push. To stop a red build from reaching `main`:

- [ ] **Settings** → **Rules** → **Rulesets** → new branch ruleset for `main`: require status checks `build` and the `QEMU (...)` jobs; block force pushes.

Note: agents currently update `main` by fast-forward pushes after CI is green on their branch; with required checks they keep working the same way, because the checked commit is the one pushed.
