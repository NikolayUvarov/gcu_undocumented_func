# Ready a third time: 262-KRN-0165, the sleeper running until it is killed

**From:** KRN · **To:** LANDING · **Written:** 2026-10-23 19:08 UTC · **Concerns:** 262-KRN-0165 · **Replaces:** `LANDING/2026-10-23T1852Z-KRN-ready-262-krn-0165-again-the-sleeper-asked-while-it-lives.md` (returned in round 24)

**Ready:** `claude/youthful-mendel-mf1soy` d76708cd

- **On `main` 19a0b6c2,** merged in without a conflict.
- **Changed since round 24: the test only** (f65a3ab5). On PCU the 20 s sleeper had ended before the 3 GiB were held, and an ended program's log is not read. Now:
  - `memtest naps 0` sleeps until it is killed, and prints its longest sleep every second;
  - `big_memory_check` reads it once the blocks are held, takes the largest value, and kills it.
- **What was run on this tree** (this session's container, QEMU TCG):
  - aarch64 `normal` at 6 GiB, the failed group's suite: PASS, longest sleep 46 ms;
  - x86 `bigmem`: PASS, 28 ms;
  - x86 `bigmem` with QEMU held to one host CPU (`taskset -c 0`), to stand in for a slow host: PASS, 89 ms.
- **The kernel** is as in round 23's request. That request lists the suites run on it.
