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

## 3. Copyright holder in LICENSE-MIT

`LICENSE-MIT` says `Copyright (c) 2026 The MIND Core contributors`. That is valid, but naming the person who owns the project is clearer for users and for programs such as Claude for Open Source.

- [ ] Decide the line, for example `Copyright (c) 2026 <your name> and the MIND Core contributors`, and tell an agent (or edit `LICENSE-MIT` yourself). Apache-2.0 needs no change.

## 4. Hand issue 051 to the tools session

The tools branch (`claude/wizardly-franklin-kec1a9`) has diverged from `main` and duplicates MIND IDL v0.2, `STAT`, input events and endpoint badges; issue numbers 032–050 collide.

- [ ] Tell the session that develops the tools to read and do [issues/051](../issues/051-merge-main-into-tools.md) (merge `main` into its branch, reconcile duplicates, renumber its issues, then fast-forward `main`).
- [ ] Until it is done, do not start new kernel or ABI work in the tools session.

## 5. Optional: protect `main`

CI (`.github/workflows/ci.yml`) runs on every push. To stop a red build from reaching `main`:

- [ ] **Settings** → **Rules** → **Rulesets** → new branch ruleset for `main`: require status checks `build` and the `QEMU (...)` jobs; block force pushes.

Note: agents currently update `main` by fast-forward pushes after CI is green on their branch; with required checks they keep working the same way, because the checked commit is the one pushed.
