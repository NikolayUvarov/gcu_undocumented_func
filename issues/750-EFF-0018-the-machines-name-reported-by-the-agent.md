# 750-EFF-0018 — The machine's name, set on the machine and over the cable, reported by the agent

**Type:** session · **Owner:** `EFF` · **Priority:** P2 · **Status:** open (2026-10-28) · **Blocked by:** `APP`: the machine's name as a system setting with its command; `KRN`: how a service reads it (both asked on `internal-dev-messages`, 2026-10-28) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-2.3, MC-10.2

## Problem

The maintainer (2026-10-28): what is needed is not the agent's name but the machine's name, set on the machine itself.

- **MIND Core has no name for the machine.** Each part that shows one has its own:
  - the Effector agent reports `effector.conf`'s `hostname` line, or since 750-EFF-0017 one made from the processor's name when that line is another machine's (`mind-n100` on the DEXP);
  - DHCP gives the router no name;
  - the link answers every hello as "MIND Core link".
- **Nothing remote can change the agent's line:** the server cannot, `link` cannot write `data/effector/`, and the shell refuses its file commands to the link's client.

## Plan (proposed to the tracks in `ALL/`, 2026-10-28)

1. **The setting (`APP`):**
   - one name for the machine, kept on the disk (proposed: `data/hostname`, one line);
   - an RFC 1123 label: 1 to 63 of `[a-z0-9-]`, neither first nor last a hyphen, so DHCP and DNS take it as it is;
   - `mindcore` when none is set.
2. **Setting it (`APP`):**
   - `hostname` and `hostname <name>` in the shell: the user's own at the console, asked from a program's client, and at once from the link's client in developer mode (the maintainer's choice of 2026-10-22);
   - a field in `wm`'s Settings.
3. **Reading it (`KRN`):** a service without a client of `data/` (the agent's own scopes, `link`, `netstack`) reads it through one service. Proposed: `sysmon`'s system information, which `effector_gw` already holds.
4. **The agent (`EFF`, this task):**
   - its heartbeat reports the machine's name, read again with each telemetry sample, so a change shows within 30 s without a restart;
   - `effector.conf`'s `hostname` line and 750-EFF-0017's name of the machine's own serve only while no name is set.
5. **The others:** `NET` sends it in DHCP (option 12); `LNK` says it in the link's hello and on `effector485`'s page (800-LNK-0028).

## Acceptance criteria

- [ ] The `effector` QEMU suite: a name set by `hostname` is in the next heartbeats without a restart; with none, the agent reports what it reports today.
- [ ] On the DEXP:
  - `effector485 submit shell line="hostname <name>"`;
  - the server lists the agent under that name within a minute.

  Evidence for the DEXP only (MC-12.1).

## Related

[750-EFF-0017](750-EFF-0017-an-identity-of-the-machines-own.md), `800-LNK-0028`, `800-LNK-0015` (the shell over the link).
