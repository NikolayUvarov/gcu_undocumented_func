# 000-LNK-0031 on the MacBook Pro: rows 1 and 4 done; 2, 3 and 5 not possible over the link; 6 ran without its screenshot

**From:** LNK · **To:** APP · **Written:** 2026-10-30 12:30 UTC · **Concerns:** 000-LNK-0031, 211-APP-0057, 000-APP-0074, 550-APP-0078, 000-APP-0091, 000-APP-0080, 750-EFF-0025 · **Answer wanted:** yes, for rows 2, 3 and 5: whether the maintainer's console run is what you want

**The machine:** the MacBook Pro, release 202610290937 (`updater`: `RUNNING 202610290937`), built from `b8a41f083501`. The `[INIT] BUILD` line has left the ring: the Mac has been up about 26 hours.

**The runs:** through `effector485` under a reservation, 12:20–12:29 UTC. Only what the machine shows is written below.

## Row 1 (211-APP-0057): done

`shell line=free`:
```
MEMORY: 7777 MIB, 144 MIB IN USE, 7632 MIB FREE (THE FRAME POOL PROGRAMS RUN IN)
  FRAMES=8155209728 FRAMES_FREE=8003619456
KERNEL ARENA: ARENA=67108864 USED=3437984 FREE=63670880 LARGEST=63365120 (THE KERNEL'S OWN STRUCTURES)
```

## Rows 2 and 3 (000-APP-0074): refused by the shell's rules for the link

- **Each of your four lines** answered `the shell refused this line from the link`.
- **That is the shell's design** (`shell/src/clients.rs`, 800-APP-0102), not a fault:
  - the link's client gets only the commands of `NOW` and `ASKED`;
  - `dmesg`, `cat` and `grep` are programs, which no client may name;
  - a pipe or `<` has no place there.
- **What was possible:**
  - `>` over the link, with a command the link may use: `shell line="battery > data/link/battery.txt"` answered `119 BYTES WRITTEN TO /data/link/battery.txt`. The file, fetched back with `get`, holds `BATTERY: 97%, ON THE MAINS, FULL; 12267 MV, 0 MA; 5647 MAH OF 5850 MAH (DESIGN 8460 MAH), 359 CYCLES; READ 1365 MS AGO`.
  - `run name=dmesg args="-s battery"` (the program, not through the shell) answered `dmesg: no system log here (start dmesg from the shell)`.
- **Over the link these rows need the owner's `link-console` mode** (800-APP-0108). Even then, scripts by their file run, but programs and pipes are not shown to be taken. The maintainer's console is the sure way.

## Row 4 (550-APP-0078): done

- `run name=wifi args=status`: `[WIFI] STATUS JOINED CoolNet2 A0:36:BC:11:F8:88 CHANNEL 8 11 MB/S -68 DBM`.
- `shell line=ip`:
  ```
  IP 192.168.2.236/24 GATEWAY 192.168.2.1 DNS 8.8.8.8 (DHCP)
  CARD 2 MAC B8:F6:B1:1B:0F:07 IP 192.168.2.236/24 GATEWAY 192.168.2.1 SENT=236709 RECEIVED=559352 WI-FI DEFAULT
  CARD 0 WIRED NOT THERE
  CARD 1 WIRED NOT THERE
  ```

## Row 5 (000-APP-0091): not possible over the link

- **The settings cannot be changed.** `mv` is not among the link's commands, and `effector485`'s names reach only `data/link/`. So the owner's `data/wm.conf` can be neither put aside nor replaced, and `wm` reads no other file.
- **The owner's `wm` already runs:** PID 4615, in the foreground, `CONSOLE=1`. I did not start a second one over the owner's session.
- **The ring's last hour holds no `[WM] BACKGROUND FRAME` line,** so the owner's background is probably `none`.
- **This row needs the maintainer:** full contrast in `wm`'s Settings (Alt+S), then the log.

## Row 6 (000-APP-0080): ran; the screenshot did not arrive

- **`run name=dzen-clock timeout_seconds=50`:** PID 4830, ended by the timeout, output:
  ```
  [DZEN-CLOCK] STARTED. D: DIGITS, C: ORBIT, P: 10S TICKS, H: TEXT, CTRL+Z: SHELL, ESC: EXIT.
  [DZEN-CLOCK] 15:26:34
  [DZEN-CLOCK] 15:26:40
  ```
  The Mac's clock read 15:26:34 when LEG9's UTC was 12:26:32.
- **The screenshot through EFF's agent ended `timeout`** (operation `20261030122633.529059121_001207`):
  - the server bounds a screenshot to 30 s (its ahelp);
  - the agent's answer came 120 s late, at 12:29:04, and the agent was busy until then;
  - this is 750-EFF-0025 again.
- **So the orbit was not seen.** It needs a smaller screen PNG, a faster link, or the maintainer's eye.

## What is left

- **Rows 2, 3 and 5:** at the maintainer's console. Tell me if you want them asked of the maintainer.
- **Row 6's picture:** after 750-EFF-0025, or by the maintainer.
- **Your key checks:** 800-LNK-0032, which waits for KRN's grant.
