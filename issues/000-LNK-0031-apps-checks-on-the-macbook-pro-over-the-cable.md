# 000-LNK-0031 — APP's checks on the MacBook Pro over the cable: six runs through `effector485`

**Type:** hardware runs · **Owner:** `LNK` · **Priority:** P2 · **Status:** open (2026-10-30) · **Blocked by:** the cable on the MacBook Pro (it was on the DEXP on 2026-10-30) · **Main task:** — · **Constitution:** MC-12.1, MC-12.2

**Asked by:** the tools track (`APP`) in `issues/requests-LNK.md`, 2026-10-30, section 1. The maintainer's word (2026-10-30): the Mac's checks go to the tracks that own the ways into it.

## Problem

APP's session runs in the cloud and does not reach `effector485`, which stays local by design ([effector485.md](../docs/link/effector485.md), the maintainer, 2026-10-30). Six of its checks need runs on the Mac over the cable.

## Plan

**The runs,** none of which restarts the machine or replaces a service:

| # | For | `effector485` operations | What APP expects |
|---|---|---|---|
| 1 | [211-APP-0057](211-APP-0057-top-and-free-show-the-machines-memory.md) | `shell line=free` | the machine's memory, about 7.6 GiB (7768 MiB), not 64 MB |
| 2 | [000-APP-0074](000-APP-0074-redirection-and-pipes.md) | `shell line="dmesg -s battery > data/battery.txt"`, then `shell line="cat data/battery.txt"` | the first prints nothing; the file holds the battery service's lines |
| 3 | 000-APP-0074 | `shell line="dmesg -s battery \| grep -c BATTERY"`, then `shell line="grep -c BATTERY < data/battery.txt"` | the same count twice, no error |
| 4 | [550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md) | `run name=wifi args=status`, then `shell line=ip` | the network the station joined, and `card2`'s address |
| 5 | [000-APP-0091](000-APP-0091-wms-background-by-rows.md) | the owner's `data/wm.conf` put aside; `background = abstract`, `contrast = 100`; `run name=wm timeout_seconds=180` for at least 70 s, with a screenshot through EFF's agent; its `[WM] BACKGROUND` lines; `wm` ended; the owner's settings back | `[WM] BACKGROUND FRAME … MS ON AVERAGE, … MS AT MOST`, within a few frame times of each other, at full contrast, on 2880 × 1800 |
| 6 | [000-APP-0080](000-APP-0080-dzen-current-hour-sector.md) | `run name=dzen-clock`; a screenshot through EFF's agent; the Mac's time noted; ended with `shell line="kill <pid>"` | the orbit's zero tick points to the corner of the hour now |

- **What goes back to APP:** each operation's output and the stick's `[INIT] BUILD` line, posted to `APP/` on `internal-dev-messages`.
- **What is recorded:** only what the machine shows.

## Acceptance criteria

- [ ] The six runs done on the MacBook Pro, their outputs posted to APP with the build line.
- [ ] Any operation the link refused named with its refusal.

## Related

[800-LNK-0032](../issues-done/800-LNK-0032-keys-and-the-pointer-over-the-link.done) (APP's checks that need keys), [effector485.md](../docs/link/effector485.md).
