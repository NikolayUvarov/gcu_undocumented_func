# Contributing to MIND Core

MIND Core is a capability microkernel and its ring-3 services in Rust for UEFI machines (QEMU first) on two architectures, each with its own platform profile:

- x86-64: [`x86-64/QEMU-0`](docs/profile/README.md);
- aarch64: [`aarch64/QEMU-virt-0`](docs/profile/aarch64/README.md), which states only where it differs from the x86-64 profile.

Contributions of code, tests, documentation and reviews are welcome.

## Where to start

- [ROADMAP.md](ROADMAP.md) ([Russian](ROADMAP_RU.md)): order of work and which parts can be developed in parallel.
- [issues/](issues/README.md): tasks scheduled now. Each task names its acceptance criteria. Pick one, or open a GitHub issue to propose a new one.
- [Constitution v1.6](constitution/EN/MIND_CORE_Constitution_v1.6.md): normative requirements (`MC-<article>.<clause>`); [docs/profile](docs/profile/README.md) says which of them the code meets and with what evidence.
- [docs/api](docs/api/README.md): system calls, `libmind`, service interfaces.
- [AGENTS.md](AGENTS.md): working through a coding agent — tracks and their issue ranges, branches, the landing queue to `main`, coordination between tracks.
- [TRACKS.md](TRACKS.md): the registry of tracks — codes, task numbers `NNN-TRK-MMMM`, ranges, owners; open tracks can be taken in parallel.

## Build and test

```bash
./01_prepare_env.sh          # Rust nightly and targets (install QEMU and OVMF with your package manager)
./02_build.sh                # all crates into usb_root/
scripts/host_tests.sh        # every host test, as CI runs them; any compile or test that fails stops it
python3 tests/idl_test.py    # generated MIND IDL bindings are up to date
python3 tests/qemu_smoke.py --qemu qemu-system-x86_64   # QEMU suites (see README for the test ELFs)
ARCH=aarch64 ./02_build.sh --fixtures                   # the same crates for aarch64, into aarch64_root/
python3 tests/qemu_smoke.py --arch aarch64              # the QEMU suites on virt
scripts/ci_local.sh          # every CI group on this machine, with a PASS/FAIL table
```

A change is ready when the suites it touches pass; a kernel change runs all QEMU suites on 4 CPUs and the SMP, isolation, heap and services suites on 1 CPU, and a change to aarch64 also runs the aarch64 groups.

## Rules

- **Commits:** one task per commit where possible, a message saying what changed and why; reference the issue number.
- **Getting to `main`** (the maintainer's decision, 2026-10-21):
  - code lands through the landing queue: a request in `LANDING/` on the `internal-dev-messages` branch, then one gated candidate for every pending request, which only the integrator pushes ([AGENTS.md](AGENTS.md), section 4, `scripts/land.sh`);
  - a text-only commit goes to `main` directly;
  - a pull request from a fork is reviewed, then landed the same way.
- **Comments:** English, one line, the essence only. Match the style of the surrounding code.
- **Refusals name their clause** (850-KRN-0146). A log line that refuses a request because a Constitution requirement forbids it ends with that clause, for example `[INIT] MODE CHANGE REFUSED: PID 42 (BADGE 2) MAY NOT TURN MAINTENANCE ON (MC-12.6)`. Name a clause only where the link is exact; a refusal that enforces none names none. Adopt the rule as you touch your own refusals; a refused system call is answered with its error and logs nothing.
- **ABI:** system call numbers, records and capability rules live in `common/abi.rs` and change only together with the kernel, `libmind`, [docs/api](docs/api/README.md) and the tests. Service interfaces change through `idl/*.wit` and `scripts/mind_idl.py` (generated files are committed).
- **Profile:** a change that alters a statement in `docs/profile` updates it in the same commit.
- **Issues:** a finished task moves from `issues/` to `issues-done/` with a Resolution section (rules in [issues/README.md](issues/README.md)).
- **Founding documents** (constitution, RFC, roadmap) exist in English and Russian and carry versions; change both.
- **No question about a tool's own purpose** (the maintainer, 2026-10-09). A program the user starts gets the devices it is for without a question: a recorder the microphone, a camera tool the camera, an editor the keyboard. Starting it is the request. A question is for what goes beyond that purpose or changes the system in a way the user did not ask for: the firmware's boot settings, the network policy. It may also choose between several devices.
- **Proprietary files** (firmware, microcode or data whose terms forbid redistribution) are never committed and never put into a disk image. A script fetches them into `proprietary/`, their separate store in the working tree, which `.gitignore` excludes; it checks them by SHA-256 and copies them onto the disk after the image is written (`data/firmware/` on the boot volume). [THIRD_PARTY.md](THIRD_PARTY.md) names the script and the source; see [AGENTS.md](AGENTS.md), section 3.
- **Legacy hardware:** code that exists only for a superseded interface is marked `LEGACY:`, isolated so that removing it is a deletion, and listed in [docs/legacy.md](docs/legacy.md); `init` reports at boot which legacy devices it found.

## Licence of contributions

MIND Core is licensed under either of [Apache License 2.0](LICENSE-APACHE) or [MIT](LICENSE-MIT) at your option. Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in the work by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions. Do not add third-party code or data without recording its source and licence in [THIRD_PARTY.md](THIRD_PARTY.md).

## Conduct and security

Participation follows the [Code of Conduct](CODE_OF_CONDUCT.md). Report vulnerabilities as described in [SECURITY.md](SECURITY.md), not in public issues.
