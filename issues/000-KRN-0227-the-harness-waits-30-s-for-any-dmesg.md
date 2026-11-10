# 000-KRN-0227 — The QEMU harness waits 30 s for any `dmesg`, not 8 s

**Type:** tests (the QEMU harness, `tests/qemu_smoke.py`'s `VM.command`) · **Owner:** `KRN` (CI) · **Priority:** P2 · **Status:** in progress (step 1 landed in round 107, `main` 6989bbe6; step 2 made, the gate left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track A · **Constitution:** MC-12.1, MC-12.2

## Problem

- **Round 106's gate** (candidate 7415c9a1) failed the group "x86: network, TLS and Effector" once, on x32-128's lane `x32`, while the lane `x32-2` ran other groups there.
  - **Where:** `effector_suite`'s `until`, in `vm.command("dmesg -s effector")`, which saw no prompt within its 8 s.
  - **The group passed when it ran again alone** (1169 s).
- **The CPUs' state at the timeout** (`land-7415c9a1/x32-03/mind-core-stuck-1018643-1.log`):
  - CPUs 0 and 1 in the kernel, in `scheduler::interrupt` (mapped through the build of the same tree: base 0x1cf79000 from the idle `hlt`);
  - CPUs 2 and 3 idle.
  - Nothing stuck shows. It is one sample.
- **The same class as round 102's failure** (000-NET-0063): `dmesg` reads the whole ring, 16 records to a call, and beside a busy lane that took more than 8 s. 000-NET-0063 gave the Wi-Fi checks' reads 30 s. The Effector suite's are EFF's, and other suites read `dmesg` too.
- **Seen once more, not explained:** round 105's second candidate (030813dd) failed the same group on `x32-2` beside `x32`, in `browse_check`: `vm.command("run netcheck ask:… &")`, 8 s with no prompt. That is not `dmesg`. The candidate was not gated further, so no rerun of that group exists.
- **The group's history:**
  - it passed on x32-128's lanes in rounds 97 and 103;
  - since round 105 it failed there twice, each time beside the other lane;
  - on i9 alone it passed in rounds 104 and 105.

## Plan

1. **Done here:** `VM.command`'s default wait is `DMESG_WAIT` (30 s) for a `dmesg` command, and 8 s for others as before. A check that passes a wait keeps its own.
   - The Wi-Fi checks' explicit 30 s are taken out, as the default covers them.
   - The loops that read `dmesg` again after a timeout (a kernel line inside the command's echo) wait longer per read now. Their deadlines are 8 s to 300 s, so a failing read is reported later, not missed.
2. **To watch:** whether x32-128's two lanes make the shell itself late (the `run … &` case).
   - If it comes again, the next step is the lane plan (`scripts/land.py`): that group kept off a lane whose machine runs another, or the default wait raised for every command.
   - That waits for a second case, not one (MC-12.2).

## Acceptance criteria

- The suites pass with the change on x86 and aarch64.
- Three gates pass with "x86: network, TLS and Effector" on an x32-128 lane beside the other, or the shell's lateness is shown and handled (step 2).

## Progress (2026-11-09)

- **Made:** step 1, on `claude/youthful-mendel-mf1soy`.

## Progress (2026-11-10)

- **Step 1 landed** in round 107 (`main` 6989bbe6).
- **The second case came in round 107:** the same group failed on `x32-2` beside `x32`.
  - **Where:** `slow_radio_check`'s `vm.service_logs("wlan", "[WLAN] COUNTRY ")` read nothing in its 40 tries. Not `dmesg`, and not the shell's prompt: the guest as a whole was late.
  - **It passed when it ran again alone** (1169 s), as in rounds 105 and 106.
  - **So three rounds running,** the group failed on an x32-128 lane beside the other lane, each time at another timed wait, and passed there alone each time.
- **What x32-128 runs besides our lanes,** read through Effector at 23:55 UTC, during the gate:
  - three libvirt guests (`qemu-system-x86`, up 6 days, about 12 % of a CPU together);
  - `ksmd`, the kernel's merging of equal pages;
  - the package at 62 °C, the governor `powersave`.
  - **These are not ours:** they stay as they are, and nothing here says they are the cause.
- **Step 2, made:** `scripts/land.py` keeps the group off x32-128 in the pool (`KEPT_OFF`), so it goes to i9, acc-1 or acc-2.
  - The group runs whole as before, and a failed one still runs again where it failed.
  - `tests/land_test.py` checks the rule. A dry run of the pool puts the group on acc-2.


## Related

[000-NET-0063](000-NET-0063-the-wifi-checks-dmesg-waited-8-s-on-a-loaded-host.md) (round 102, the Wi-Fi checks), [000-KRN-0225](../issues-done/000-KRN-0225-the-revocation-check-saw-no-refusal-on-a-loaded-host.done) (round 103, another check on a loaded host).
