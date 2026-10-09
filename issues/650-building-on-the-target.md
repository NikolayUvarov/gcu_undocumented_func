# 650 — Building on the target: a git client and Rust, so the system fetches its sources and builds itself

**Type:** main task · **Owner:** `DEV` track (proposed; no owner) · **Priority:** P3 · **Status:** open (proposed: the maintainer confirms the track's code and range) · **Blocked by:** stage by stage, below · **Roadmap:** none yet (a proposed track; stage 5 meets track C's reproducible builds) · **Constitution:** MC-9.1, MC-9.2, MC-12.1, MC-12.3

Asked by the maintainer (2026-10-08): a small git and Rust on the target, so the system can fetch its own sources and build itself. Opened by the kernel session with the proposed track `DEV` ([TRACKS.md](../TRACKS.md)).

## Problem

MIND Core is built only on a host today, with the nightly toolchain pinned in `rust-toolchain.toml`. Its programs are `no_std` over `libmind`. The tree is about 55 000 lines of Rust in 315 files, with about 185 third-party crates in its lock files (counted on 2026-10-08). On the target there is:

- no git client, compiler, linker or POSIX layer;
- one address space with one 64 KiB stack per task, and no threads;
- at most 1024 MiB of memory for an application (its request note);
- a 504 MiB FAT16 boot volume, and no data partition yet (211, "later").

The compiler is the hard part. rustc and LLVM are millions of lines of Rust and C++ written for a hosted OS (files, processes, threads, `mmap`), and building Rust itself takes gigabytes of RAM and hours on a fast machine. **Stages 3–5 are long-term.** Nothing in this task is a guarantee; the profile will claim only what a stage's criteria show on a stated configuration (MC-12.3).

## Plan in stages

**1. mini-git: fetching the sources, read-only.**

- A `no_std` + `alloc` Rust client: clone and fetch over the HTTPS smart protocol. It needs pkt-line, the reference advertisement, packfiles, zlib inflate, SHA-1 object IDs (git's collision-detecting variant to evaluate), and delta resolution (offset and reference deltas).
- Checkout into `vfs`. FAT keeps no file modes or symbolic links; whatever cannot be represented is reported.
- A shallow clone (depth 1) first, then fetches. Over SSH later, blocked by the SSH client ([requests-NET.md](requests-NET.md)). No push.
- An object ID says what was fetched, not who wrote it. That a commit is the maintainer's comes from a signed manifest that names it (350), not from the transport.
- *Acceptance:* in QEMU against a test server, and on the MacBook Pro against the public repository, a clone checks out a tree whose every file hashes to its blob ID.

**2. Builds without a native compiler.**

- The effector ([501](501-effector.md)) sends the fetched commit, or local changes, to a build server. The server builds with the pinned toolchain and returns an image.
- The image is signed (350) and installed only through self-update (351), as a trial with last-known-good. Which key signs such builds is a key-role question (351-UPD-0009).
- *Acceptance:* a change made on the target runs, after a trial boot, with no step by hand on the host.

**3. A POSIX layer for ported programs.**

- Files, processes (spawn, wait, exit status), threads, `mmap`, time and sockets, over `libmind`, `vfs`, `loader` and `netstack`. Rust's `std` for a MIND target (a new target triple) sits on it.
- Redox's relibc is one candidate to evaluate; if it is used, its licence goes into [THIRD_PARTY.md](../THIRD_PARTY.md).
- It needs kernel features that are `KRN` tasks. They are requests, not done, and go to [requests-KRN.md](requests-KRN.md) when the stage starts:
  - threads: several stacks and contexts in one address space, and a wait on a memory word;
  - mapping a `vfs` file into memory, with private copy-on-write pages;
  - a larger address space and memory quota than an application may ask for today, and stacks larger than 64 KiB.
- *Acceptance:* a stated set of ported C and Rust `std` test programs passes in QEMU on x86 and aarch64.

**4. The Rust toolchain.**

- rustc with LLVM: the full compiler, on stage 3, with gigabytes of RAM and a linker (`lld`).
- A lighter path to evaluate: cargo with the Cranelift back end (`rustc_codegen_cranelift`). It takes LLVM out of code generation, but the rustc front end is still a large program, and a linker is still needed.
- Redox has reported running rustc and cargo over relibc; what that took is part of the evaluation.
- *First goal and acceptance:* `libmind` and one program built on the target, cross-built tools first. The program runs, and its image is compared with the host's build.

**5. Self-hosting.**

- The whole tree built on the target from the fetched sources and the vendored crates.
- The result compared byte for byte with the host's build of the same commit with the same toolchain (350's reproducible builds). A difference is a finding, not a pass.
- *Acceptance:* identical images, or every difference explained and recorded.

## Acceptance criteria

Each stage's criteria above, met in order. The main task closes when stage 2 passes and stages 3–5 are split into issues of their own, or when all five pass.

## Related

[TRACKS.md](../TRACKS.md) (the proposed track `DEV`), [501](501-effector.md), [350](350-signed-boot-images.md), [351](351-self-update.md), [211](211-intel-pc-from-a-sata-ssd.md) (a data partition), [requests-NET.md](requests-NET.md) (the SSH client), [requests-KRN.md](requests-KRN.md), ROADMAP track C.
