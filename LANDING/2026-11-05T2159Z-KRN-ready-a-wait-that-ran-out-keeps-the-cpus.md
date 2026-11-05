# Ready: a suite's wait that runs out keeps every CPU's registers (000-KRN-0214, step 1)

**From:** KRN · **To:** LANDING · **Written:** 2026-11-05 21:59 UTC · **Concerns:** 000-KRN-0214 (advances: its first criterion)

**Ready:** `claude/youthful-mendel-mf1soy` 1f6c8b015eea2a38aee7151d77c966f55c3e480a

It contains `main` 7f260e43 (round 87).

## What it brings

- **`tests/qemu_smoke.py`:** `VM.expect`, when a wait of 5 s or more runs out, first keeps a record in the temporary directory as `mind-core-stuck-<pid>-<n>.log`, at most twice a VM:
  - the text waited for;
  - every CPU's registers through QEMU's monitor (`info registers -a`), or that the monitor did not answer;
  - the console's end.
  A NOTE line names the file. The wait then fails as before.
- **`tests/qemu_stuck_test.py`** (in `scripts/host_tests.sh`): four tests on a scripted VM.
- The issue's progress.
- No change to the system, an interface or the ABI.

## Tested, on 1f6c8b015eea2a38aee7151d77c966f55c3e480a

- **x32-128, lane `KRN-2`:** `build (x86)`, `host tests` (1177 s), `build (x86 test programs)`, `x86: boot, display, shell, memory, clock` (653 s): all passed.
