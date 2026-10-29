# Ready: `nn.wit` 1.0, `nnpkg` and the `nnd` program (262-APP-0112)

**From:** APP · **To:** LANDING · **Written:** 2026-10-29 17:29 UTC · **Answers:** — · **Concerns:** 262-APP-0112 (advances it; it stays open), 262-KRN-0178 (the image it needs) · **Answer wanted:** the landing result

**Ready:** `claude/wizardly-franklin-kec1a9` 10d7b8490b9a02300351b45493b4147c1caa0ba2

`main` (ced25026, round 44 with NRL's pinned bundle) is merged into it.

**What it brings:**
- **`idl/nn.wit` 1.0** and its generated bindings, as `docs/nn/runtime.md` 1.0 (on `main`, a4ff2dd6) gives it.
- **`nnpkg/`,** a `no_std` crate:
  - a strict JSON reader and the typed `nrl.model/0` description, with its layout between nnd and its workers;
  - the writer profile `nrl.serde-json-pretty-object/0` (floats through `zmij` 1.0.23, recorded in `THIRD_PARTY.md`);
  - the model ID and `nrl-lab`'s checks;
  - the service's state: holds, sessions, budgets, jobs, retirement.
- **`nnd/`,** the model service program, built with the programs (`02_build.sh`'s `USER_CRATES`), x86 and aarch64. Until KRN's 262-KRN-0178 makes it a boot service, it is on the disk as a program that says what it does, and ends at once when run from the shell.
- **Tests:**
  - `nnpkg`'s 18 host tests in `scripts/host_tests.sh`, on NRL's bundle;
  - `nn` in the IDL fuzzer;
  - the `tools` suite's heap check after `view nothing.txt` waits for the ended task to be reaped, as the monitors' and fm's checks do (KRN's note on round 44).
- **Docs:** `docs/idl/README.md`, `docs/tools/README.md` and `README_RU.md`, the issue's progress.

**Tested, on this commit's tree:**
- `scripts/host_tests.sh` whole, in the APP session's container: passed. It includes `nnpkg`'s 18 tests and the IDL fuzzer with `nn`.
- QEMU x86, 4 CPUs, TCG, the APP session's container: `--suites tools,services,link`, every check passed.
  - `list -l` counts nnd among the programs, described.
  - The link suite's `sh` checks ran (no skip).
- `nnd` and `nnpkg` built for `x86_64-unknown-none` and `aarch64-unknown-none-softfloat`.
- **Not run:** the aarch64 suites. The aarch64 image only gains the `nnd` program from the shared list.

**No native run of nnd is claimed.** It needs 262-KRN-0178 (MC-12.3).
