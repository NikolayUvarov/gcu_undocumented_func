# 750-EFF-0018 — The machine's name, set on the machine and over the cable, reported by the agent

**Type:** session · **Owner:** `EFF` · **Priority:** P2 · **Status:** in progress (2026-11-14) · **Blocked by:** — (000-APP-0123's `hostname` and 000-KRN-0180's `sysinfo.wit` 4.2 are in `main`) · **Main task:** [750](750-effector-agent.md) · **Constitution:** MC-2.3, MC-10.2

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

[750-EFF-0017](../issues-done/750-EFF-0017-an-identity-of-the-machines-own.done), `800-LNK-0028`, `800-LNK-0015` (the shell over the link).

## Progress (2026-11-14)

- **The parts it waited for are in `main`:** the shell's `hostname <name>` (000-APP-0123) keeps the name in `data/machine/name`, and `sysmon` answers it as `sysinfo.wit` 4.2's `hostname` (000-KRN-0180).
- **`effector.wit` 1.6.** `sample` gains `hostname: string<63>`, which the gateway reads from `sysmon` for each sample. The same version holds 64 services in both lists, KRN's request of 2026-11-12.
- **The agent.**
  - With each telemetry sample (30 s) it takes the machine's name, unless it is `mindcore`, which means none is set.
  - Otherwise its own stands: `effector.conf`'s `hostname`, or 750-EFF-0017's name of the machine's own.
  - A change is logged as `EFFECTOR: HOSTNAME <name> (THE MACHINE'S NAME | THE AGENT'S OWN)`, and the next heartbeat reports it.
- **The `effector` suite.**
  - `hostname mind-bench-1` reaches the heartbeats within a sample, with no restart.
  - `rm data/machine/name` brings back `mind-qemu`.
  - Not run yet.
