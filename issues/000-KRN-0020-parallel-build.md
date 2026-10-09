# 000-KRN-0020 — The build runs its crates in parallel, a log each

**Type:** build · **Owner:** `KRN` · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Roadmap:** track A (the gate's time) · **Constitution:** MC-12.1

## Problem

`02_build.sh` built about 60 crates, the kernel, the services, the programs and then the UEFI bootloader, one after another. The x86 build took about 5 minutes of a clean local gate, with most processors idle while a small crate compiled. All of cargo's output went into one log, so finding which crate failed and why meant reading through it.

## Plan

- **Parallel jobs.** Every crate builds in its own `target/` directory, and no crate's build reads another's output. So `scripts/build_jobs.sh` runs the cargo builds as parallel jobs:
  - one per processor at once, or `$MIND_BUILD_JOBS`;
  - the bootloader first, then the crates in `USER_CRATES` order, each crate directory once.
- **A log each.** Each job writes `code_handoff/build/<crate>.log` (aarch64: `code_handoff/build-aarch64/`).
- **The summary.** A line as each job ends: `ok` or `FAILED`, with its time. At the end, for each failed job, its `error` lines with their context and the path of its full log. The build then fails with that step named.
- **Staging and signing** run after all jobs, as before.
- **Both architectures.** `02_build.sh` (x86) and `scripts/build_aarch64.sh` use it. `ci_local.sh` and CI run them as before.

## Acceptance criteria

- A clean build of x86 and of aarch64 gives the same files in `usb_root/` and `aarch64_root/` as the sequential build, and the gate passes on them.
- A crate that fails to compile is named with its errors at the end. The build exits with an error, and the other jobs' logs are complete.
- The clean build's time is measured before and after on the same machine and recorded here.

## Related

`scripts/ci_local.sh`, `.github/workflows/ci.yml`, issue 011 (pinned toolchain).
