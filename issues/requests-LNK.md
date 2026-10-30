# Requests for the MIND Link track (LNK), not numbered yet

**Owner:** MIND Link track · **Status:** open (2026-10-30: APP's checks on the MacBook Pro through `effector485`) · **Recorded by:** the tools track (APP), 2026-10-30

The LNK track numbers its own tasks (`NNN-LNK-MMMM`), so requests from other tracks wait here. It turns each into a task and removes it from this file. The file is kept while empty because other issues link to it; a new request goes below this line.

## APP's checks on the MacBook Pro through `effector485` (APP, 2026-10-30)

Recorded by the tools track (APP), at the maintainer's word (2026-10-30). The Mac's real-machine checks go to the tracks that own the ways into it. APP's session runs in the cloud, so it does not reach `effector485` on LEG9 (`127.0.0.1:8485`).

### 1. Runs of programs and shell lines

None of these restarts the machine or replaces a service.

Please post to APP on `internal-dev-messages`:
- each operation's output;
- the stick's build, the `[INIT] BUILD` line of its log.

Write in this table only what the machine shows.

| # | For | `effector485` operations | What APP expects |
|---|---|---|---|
| 1 | [211-APP-0057](211-APP-0057-top-and-free-show-the-machines-memory.md) | `shell line=free` | the machine's memory, about 7.6 GiB (7768 MiB), not 64 MB |
| 2 | [000-APP-0074](000-APP-0074-redirection-and-pipes.md) | `shell line="dmesg -s battery > data/battery.txt"`, then `shell line="cat data/battery.txt"` | the first prints nothing; the file holds the battery service's lines |
| 3 | 000-APP-0074 | `shell line="dmesg -s battery \| grep -c BATTERY"`, then `shell line="grep -c BATTERY < data/battery.txt"` | the same count twice, no error. A line the shell refuses over the link is worth knowing too |
| 4 | [550-APP-0078](550-APP-0078-wifi-join-without-a-screen.md) | `run name=wifi args=status`, then `shell line=ip` | the network the station joined, and `card2`'s address |
| 5 | [000-APP-0091](000-APP-0091-wms-background-by-rows.md) | see below | `[WM] BACKGROUND FRAME … MS ON AVERAGE, … MS AT MOST` within a few frame times of each other, at full contrast, on 2880 × 1800 |
| 6 | [000-APP-0080](000-APP-0080-dzen-current-hour-sector.md) | `run name=dzen-clock`; EFF's agent takes a screenshot; note the Mac's time; end it with `shell line="kill <pid>"` (`shell line=ps` gives the PID) | the orbit's zero tick points to the corner of the hour now, the green one |

**Row 5, step by step:**
1. **Keep the owner's settings.** If `data/wm.conf` exists, put it aside: `shell line="mv data/wm.conf data/link/wm.conf.owner"`.
2. **Full contrast.** `put` a `data/link/wm.conf` with the two lines `background = abstract` and `contrast = 100`, then `shell line="mv data/link/wm.conf data/wm.conf"`.
3. **Run wm.** `run name=wm timeout_seconds=180`. Leave it for at least 70 s, since it logs its frame times at its first frame and then once a minute. Meanwhile EFF's agent takes a screenshot.
4. **Read and stop.** Take the `log` lines with `[WM] BACKGROUND`, then end `wm` with `shell line="kill <pid>"`.
5. **Put the settings back.** `shell line="mv data/link/wm.conf.owner data/wm.conf"`, when there was one.

If `run` cannot start a window manager, say so: the maintainer then starts `wm` by hand.

### 2. A way in for a session that is not on LEG9

- **Today** only sessions on the maintainer's laptop reach `effector485`.
- **What is asked:** a way for another track's session to submit and read operations itself, through the Effector server or a key-protected address that sessions can reach. That would let APP run section 1 without anyone's hands, and run it again on each new build.
- **The authority stays as it is:** the daemon's queue and leases, the developer mode's refusals, and `--key-file`.

### 3. A question: keys over the link

These checks need keys pressed in `wm`, which no operation of the link gives today:
- Alt+0 on the FaceTime camera's window: [211-APP-0046](211-APP-0046-a-window-back-to-its-contents-size.md), [158-APP-0043](158-APP-0043-the-camera-from-wm-and-console.md);
- the clock and the keyboard pages of `wm`'s Settings: [000-APP-0055](000-APP-0055-date-and-time-from-wms-settings.md), [000-APP-0048](000-APP-0048-wm-settings.md);
- Ctrl+Alt+F6: [000-APP-0089](000-APP-0089-screenshots-and-recordings-for-the-page.md).

Would an operation that sends key presses in the developer mode, as QEMU's `sendkey` does in the suites, be possible? It would touch the input path, so KRN and DRV are concerned too. If not, these stay the maintainer's runs.
