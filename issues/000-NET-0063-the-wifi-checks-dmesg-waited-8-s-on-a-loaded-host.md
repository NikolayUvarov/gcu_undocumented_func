# 000-NET-0063 — The Wi-Fi checks' `dmesg` waited 8 s on a loaded host (round 102)

**Type:** tests (the `net` suite) · **Owner:** `NET` · **Priority:** P2 · **Status:** in progress (the wait raised, on `claude/youthful-mendel-mf1soy`; the gate left) · **Blocked by:** — · **Main task:** — · **Roadmap:** track D · **Constitution:** MC-12.1, MC-12.2

## Problem

- **Round 102's gate** (candidate c6931ded) failed the group "x86: RAM above 4 GiB" on x32-128 once:
  - in `world_country_check` (550-NET-0035), `vm.command("dmesg -s wifi")` saw no prompt within its 8 s;
  - the console showed `STARTED PID=11 NAME=dmesg FOREGROUND` and nothing after it.
  - The group passed when it ran again alone.
- **The CPUs' state** at the timeout (`mind-core-stuck-1552372-1.log` in the lane's logs):
  - CPU 0 was in user mode at 0x80000026ca, which in the same build's `dmesg` is in `List<log::Entry, 16>::decode`;
  - CPUs 1 to 3 were idle.
  - So `dmesg` was still reading the log, not stuck.
- **Why it was slow:**
  - Without `-n`, `dmesg` reads the whole ring (up to 2048 records), 16 records to a call to `logd`, and filters by `-s` in the program.
  - Meanwhile the lane `x32-2` was building on the same machine (`build (x86)` ended a minute before), so QEMU's TCG had less of the host.
- **The same 8 s wait** is in seven other `dmesg -s` reads of the Wi-Fi checks.

## Plan

- **The checks wait 30 s** for `dmesg -s NAME` (`DMESG_WAIT` in `tests/qemu_smoke.py`), as they do for the other commands that read the log.
- **`dmesg` itself does not change** here. It is the tools track's (069), and the time is the host's load (MC-12.1), not a fault found in it. Possible remarks for its owner:
  - `-s` could be filtered by `logd`;
  - without `-f`, `dmesg` could stop at the ring's end as it was at its start.

## Acceptance criteria

- The `net` suite passes on x86 and aarch64 with the longer wait.
- A gate passes with the group beside another lane on the same machine.

## Progress (2026-11-09)

- **Made:** `DMESG_WAIT = 30` and the eight `dmesg -s wifi|wlan|wlansim` reads of the Wi-Fi checks given it.

## Related

[550-NET-0035](../issues-done/550-NET-0035-the-stations-country.done), [069](../issues-done/069-logd-dmesg.done).
