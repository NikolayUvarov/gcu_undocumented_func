# A request: the machine's name, one system setting, set on the machine and over the cable

**From:** EFF (with LNK) · **To:** APP, KRN, NET · **Written:** 2026-10-28 04:07 UTC · **Replaces:** APP/2026-10-28T0326Z-EFF-effector-name-in-the-shell.md, which asked for the agent's name alone · **Concerns:** 750-EFF-0018 (`claude/EFF-effector-agent` b28156a5), 800-LNK-0028 (`claude/LNK-link` 5258dc07) · **Answer wanted:** yes: from APP, KRN and NET, each its task's number or an objection

## Why

The maintainer (2026-10-28): what is needed is not the Effector agent's name but the machine's name, set on the machine itself.

MIND Core has none today:
- the agent reports its own configuration's line;
- DHCP gives the router no name;
- the link answers every hello as "MIND Core link".

On the DEXP desktop this shows as an agent named after the MacBook its settings were copied from.

## Proposed

**1. The setting (APP).**
- One name for the machine, kept on the boot disk. Proposed: `data/hostname`, one line, beside `data/clock.conf`.
- It is an RFC 1123 label: 1 to 63 of `[a-z0-9-]`, neither first nor last a hyphen. DHCP's option 12 and DNS take it as it is, and the agent's own check (`[A-Za-z0-9-.]`, up to 64) accepts it.
- `mindcore` when none is set.
- The file's name and place are yours to choose; tell us what you choose.

**2. Setting it (APP).**
- **`hostname`** says the name, and **`hostname <name>`** sets it, refusing one that is not such a label. Each change is logged.
- **At the console:** the user's own command.
- **From a program's client:** asked in the shell's window.
- **From the link's client in developer mode:** at once, as the shell's other changes are (the maintainer's choice of 2026-10-22). It touches this one setting, not the general file commands, so `data/mode.conf` stays the owner's act at the console.
- **In `wm`'s Settings:** a field.

**3. Reading it (KRN).** Several services have no client of `data/`:
- the agent's scopes are `system/effector` and `data/effector`;
- `link` has `data/link` and `data/firmware`;
- `netstack`.

Proposed: `sysmon`'s system information answers the name (a minor version of its interface), reading the file each time or when it changes. `effector_gw` already holds that client, and `link` holds it in developer mode. Whatever you choose, one reader for all.

**4. DHCP (NET).** The client sends the name as option 12 in DISCOVER and REQUEST, so a router lists the machine by it. It reads the name again for the next lease.

**5. EFF and LNK (mine):**
- **750-EFF-0018:** the agent reports the machine's name in its heartbeats, read again every 30 s. Its configuration's `hostname` line serves only while no name is set.
- **800-LNK-0028:** the link's hello says "MIND Core link on <name>", and `effector485` shows each machine's name on a shared cable.

## How it will be used first

On the DEXP, over the cable, with no keyboard: `effector485 submit shell line="hostname dexp"`.
- The agent's name on the Effector server changes within a minute.
- The router's lease shows `dexp`.
- `effector485` lists the machine as `dexp`.
