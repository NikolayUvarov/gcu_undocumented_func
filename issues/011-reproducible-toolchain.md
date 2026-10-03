# 011 — Reproducible build: `rust-toolchain.toml`, `Cargo.lock`, workspace

**Type:** infra · **Priority:** low · **Status:** open
**Affects:** repository root, `01_prepare_env.sh`, `.gitignore`

## Problem

- `01_prepare_env.sh` runs `rustup default nightly` — it changes the user's global toolchain, and the nightly is "floating" at that: today `1.100.0-nightly (2026-09-14)`, tomorrow something else. The `-Z relax-elf-relocations` flag (issue [001](001-flat-binary-entry-offset-and-got-call.md)) and `abi_x86_interrupt` are nightly features; their behaviour may change.
- `Cargo.lock` is in `.gitignore` — for binary crates it is customary to commit it; `uefi` is pinned to `=0.27.0`, but `bitflags`, `log`, `syn` etc. are pulled in at their latest versions.
- Three separate crates without a workspace: three `target/` directories, three lock files, `BootInfo` duplicated.

## Plan

1. A root `rust-toolchain.toml`:
   ```toml
   [toolchain]
   channel = "nightly-2026-09-14"
   components = ["llvm-tools-preview", "rust-src"]
   targets = ["x86_64-unknown-none", "x86_64-unknown-uefi"]
   ```
   and remove `rustup default nightly` from `01_prepare_env.sh` (rustup will pick up the file by itself).
2. Remove `Cargo.lock` from `.gitignore`, commit the lock files.
3. Workspace: a root `Cargo.toml` with `members = ["bootloader", "kernel", "app", "mind-abi"]`; different targets for members — via `.cargo/config.toml` in each crate (as now) or `-Z per-package-target`. If the workspace gets in the way (different targets in a single `cargo build`), keep separate crates but hook up the shared `mind-abi` as a `path` dependency.
4. CI (GitHub Actions): `02_build.sh` + the checks from issue 001 (`Entry 0x0`, no relocations).

## Acceptance criteria

- A clean clone + `rustup` builds with a single command without manual steps.
- `git status` is clean after the build (artifacts only in ignored paths).
