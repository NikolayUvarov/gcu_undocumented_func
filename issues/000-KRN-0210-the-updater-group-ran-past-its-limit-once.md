# 000-KRN-0210 — The x86 updater group ran past Effector's limit once, and its logs were lost

**Type:** the landing gate (`scripts/land.py`), and a suite to find · **Owner:** kernel session · **Priority:** P2 · **Status:** in progress · **Blocked by:** — · **Roadmap:** the landing gate · **Constitution:** MC-12.2

## Problem

**Seen once.** Round 81's gate ran candidate e860d0bc on x32-128, lane `x32`, the group `x86: the updater and an update through the link`.
- The group started at 16:56:13 UTC.
- Effector ended it at 17:54:33, after its 3500 s: `command timed out`. It printed nothing.
- The group usually takes about 600 s. Run again alone on the same lane, it passed in 604 s, so the gate was green.
- Beside it, lane `x32-2` ran other groups on the same machine, as in every gate since 2026-11-03.

**Its logs are lost:**
- the group's command copies the logs only after `scripts/ci_local.sh` ends, and Effector's limit killed it first;
- the run again used the same lane's `TMPDIR`, and `ci_local.sh` empties its log directory at its start.

**What is not known:** which suite hung, and on what. Its suites are `updater` (raw images, the release server, reboots) and `link-update` (the link's tool over a pty and TCP). This is not a root cause (MC-12.2).

## Plan

1. **Keep the logs of a group that hangs.**
   - `land.py` runs each group in a session of its own and ends it, with every process in it, after 3300 s (`bounded`, `GROUP_LIMIT`).
   - It then prints the group's FAIL row and copies its logs, the suites' own `mind-core-*.log` among them.
2. **Reproduce:** the group alone, 10 times, on x32-128 with the other lane busy. Then find the suite, and the wait in it, from its logs.
3. **Fix the wait** if it is the test's or the code's; or give the wait a limit of its own, so that it fails with its own message.

## Acceptance criteria

- `land.py`'s test runs a group that hangs with a child of its own. It checks that both end at the limit and a FAIL row is printed.
- The cause is named, with the log that shows it, or 10 runs in a row pass on x32-128 beside a busy lane and that is recorded.

## Progress

2026-11-04:
- **`bounded` and `GROUP_LIMIT`** are in `scripts/land.py`. The group's command copies `mind-core-*.log` too, and shows the log's tail for a group its limit ended.
- **The test** is `tests/land_test.py`'s `test_a_group_past_its_limit_ends_with_its_processes_and_a_fail_row`, run with `sh`:
  - a hung group with a child of its own ends at the limit, and its FAIL row is printed;
  - one that ends in time prints nothing.

  With the kills taken out of `bounded`, it fails.

2026-11-05, on the gate's machines (x32-128 in `sh`, acc-1, acc-2 and PCU), through Effector:
- **The command:** `bounded` with a limit of 4 s, around a command that hangs with a child of its own.
- **On each machine:**
  - the FAIL row came at 4 s, and no process of the group was left;
  - a command that ends at once printed nothing, and its watchdog went with it.
- `setsid` and `/bin/kill` (procps) are on all four.

**Left:** steps 2 and 3.

2026-11-05, step 2: the group alone in lane `KRN` on x32-128 (`main` f81cd4d9), three runs per Effector operation:
- **Runs 1–6 and 10–12 passed,** 603–675 s each.
- **Run 7 hung, the second time seen:**
  - it started at 10:40:14 UTC, two minutes after round 85's gate began on lane `x32-2` of the same machine (there `x86: network, TLS and Effector` from 10:40:20);
  - its output stopped at the group's first line;
  - Effector ended the operation at 11:38:33, after its 3500 s, and runs 8 and 9 never started.
- **Its logs were lost again.** The operation copied them only after the group, and run 10's `ci_local.sh` emptied the log directory at its start.
- **Both hangs came beside a busy second lane,** running different groups there: `x86: boot, display, shell, memory, clock` in round 81, `x86: network, TLS and Effector` here.
- **The suites' waits** (`updater`, `update`, `install`, `linkupdate`):
  - each has a limit of its own, the longest 1500 s (`update_suite`'s trial boot), so one wait that runs out fails the group;
  - two have none: the `cargo build --quiet` of `mindlink` (`mindlink_tool`, and the link suite's).

  That is a candidate, not a cause (MC-12.2).

**Next:** runs 13–24, each in a session of its own with a limit of 1050 s, with lane `KRN-2` kept busy by x86 groups. At the limit the run:
- records the session's processes (`ps` with `wchan`);
- interrupts `qemu_smoke.py` with `SIGINT`, so its traceback names the wait and its `finally` blocks write the serial logs;
- copies the logs, then ends the session.

The second criterion is not met: a run hung within the first ten.

## Related

[000-KRN-0208](000-KRN-0208-a-run-after-forty-kills-waited-past-8-s.md) (another wait seen once in a gate); `docs/effector.md`, section 1 (a failed group runs once more, alone).
