# Ready: the Effector agent's session (first slice), mind::zip, the gateway's core and the modes' design

**From:** EFF · **To:** LANDING · **Written:** 2026-10-21 15:07 UTC · **Concerns:** 750-EFF-0003, 0004, 0005, 0006, 0007, 0013; TRACKS.md 1.9

**Ready:** `claude/EFF-effector-agent` 732c264bd65e33b87c672475867dfa932267f6e9

## What it closes or advances

- **Closes:**
  - 750-EFF-0003: `idl/effector.wit` 1.0 and its generated bindings, the policy grammar and decisions (`effector_gw/src/policy.rs`), the audit line, the threat analysis;
  - 750-EFF-0004: the bounded JSON writer `effector/src/json.rs`;
  - 750-EFF-0005: `mind::zip`, a bounded ZIP reader in `libmind`.
- **Advances:**
  - 750-EFF-0006, first slice: the program `effector`, started from the shell. It heartbeats, polls and ACKs against the fixture, journals every answer and denies every command. It is in the x86 and aarch64 builds and has a QEMU suite, `effector`, added to CI's x86 boot/network group and the aarch64 files/network/TLS group (`ci.yml`, `ci_local.sh`).
  - 750-EFF-0007, part: the gateway's decisions and effects as a library (`effector_gw/src/gateway.rs`), tested on the host. It is not a boot image yet.
  - 750-EFF-0013, text: the agent only in the development and maintenance modes (KRN's 750-KRN-0143).
- **Text:** TRACKS.md 1.9 names EFF's owner.
- **Untouched:** the ABI (`common/abi.rs`), `init`, the kernel, the shell.

## What was tested, and where

All runs were local, on the maintainer's laptop (LEG9), in a network namespace.

- **On 732c264b (`main` 79bc4bd2 merged), the tests this change touches:**
  - `build (x86)`, `build (aarch64)` and `host tests`, all PASS. The host tests include `effector_{session,policy,json,wire,config,journal,gateway}`, `zip`, `idl_fuzz` (which lists `effector.wit`) and `tests/effector_server_test.py`.
  - `tests/qemu_smoke.py --suites effector` on x86_64 and on aarch64, both PASS.
- **Earlier full runs of `scripts/ci_local.sh`** (before I knew a track no longer runs them):
  - **9e4e6738:** all 33 groups PASS.
  - **36e39e03 (`main` bdc3ce8 merged):** 4 groups failed.
    - "keys, shell, tools", "files and block writes" and "no PIT" each passed when run again.
    - "x86: USB image" failed twice inside a full sequence and passed 4 of 4 times alone.
    - `main` bdc3ce8 passed it 5 of 5 times.
    - The failing step comes right after a second stick is hot-plugged while `vfs_server` holds the first as not answering. The guest then does not echo the next `ps` within 8 s.
    - None of this branch's code runs in that test. The image differs from `main`'s only by `effector.elf`. I have not explained the failure, so it is reported here as it is.

## Merge notes

`issues/README.md` conflicted with `main` in its tables only. Both sides' rows were kept.
