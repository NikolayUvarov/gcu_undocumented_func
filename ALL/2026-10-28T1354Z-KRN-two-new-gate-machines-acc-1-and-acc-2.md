# Two new machines for builds, tests and gates: acc-1 and acc-2

**From:** KRN · **To:** ALL · **Written:** 2026-10-28 · **Concerns:** docs/effector.md, sections 1, 4 and 6 (`main` 4ecde4a8)

The maintainer added three machines to Effector today, for gates. Two are ready; one is not.

## Ready: `acc-1_e9f12f8b0550` and `acc-2_a01a5832facb`

- **The machines.** Each is an Intel Core i3-12100 (4 cores, 8 threads) with 31 GB of RAM, on Ubuntu 22.04 itself, with `parallel_limit` 2.
- **How to run work there.**
  - The agent runs as `un`: give no `run_as`.
  - Commands run in `sh`, so leave out `set -o pipefail` or start the script with `bash -s`.
  - Clone into `/home/un/mind-core/<TRK>/` and set `TMPDIR=/tmp/mind-<TRK>` (rule 4).
- **What is installed.** `scripts/setup_build_env.sh` ran as root at the maintainer's request (section 6):
  - QEMU 8.2.2 from qemu.org in `/opt`;
  - OVMF and AAVMF 2022.02, `swtpm` 0.6, OpenJDK 11, gcc;
  - rustup with our toolchain is in `~un` and shared with every track.
- **Speed.** The long x86 groups take as long as on un-rizen with one lane per machine. Examples: keys 1010 s, files 1047 s, services 561 s, RAM above 4 GiB 1092 s. The table is in section 1.
- **What Python 3.10 keeps off them** (Ubuntu 22.04):
  - **the host part**, as `scripts/models.py` needs `tomllib`. `ci_local.sh` will name it as a missing tool once the kernel track's next landing reaches `main`;
  - **the groups with the Effector suites**: the fixture's `parse_qs` refuses an empty query on 3.10, and EFF is asked for a one-line fix in `issues/requests-EFF.md`.
- **The landing gate** uses them from round 40, with the preset `wide`.

## Not ready: `s64_ff4f6639826a`

It runs Fedora 27, with QEMU 2.10, Python 3.6 and no compiler, and `sudo` there needs a password. It stays out of gates until its system is newer, as the maintainer decided.
