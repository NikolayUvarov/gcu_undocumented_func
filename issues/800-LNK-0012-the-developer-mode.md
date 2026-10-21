# 800-LNK-0012 — The developer mode: `dev` in `link.conf`, for debugging a machine over the link

**Type:** service, tool · **Owner:** `LNK` · **Priority:** P1 · **Status:** in progress · **Blocked by:** `KRN`: the grants `init` gives `link` with `dev` (LNK's request of 2026-10-21 on `internal-dev-messages`); `NET`: how the policy broker treats the link's flows (asked the same day) · **Main task:** [800](800-mind-link.md) · **Constitution:** MC-2.3, MC-3.11, MC-10.2, MC-12.1, MC-12.3

## Problem

The maintainer asked on 2026-10-21: "Can we add debug developer mode to allow full normal debug using this effector link? Add necessary words to link.conf to be able do the tests."

The test at hand is the MacBook Pro's Wi-Fi: joining, pinging the gateway, restarting with a new driver. A program started over the link gets an application's standard grants ([800-LNK-0009](800-LNK-0009-programs-started-for-the-master.md)). It cannot reach the network or the station, and nothing over the link restarts the machine. So each step needs the maintainer at the keyboard.

## Plan

1. **The word `dev`** in `link.conf`; the maintainer's `build-and-write.sh --link-dev` writes `baud=460800 run dev`. It is for the maintainer's own machines: whoever holds the cable may then debug the machine as its user would.
2. **What `init` grants `link` with it** (KRN, asked):
   - the network policy broker's client;
   - the Wi-Fi station's configuration client with `BADGE_WIFI_CHANGE`;
   - the system monitor's client;
   - a lifecycle client badged `BADGE_REBOOT`.
3. **`link` with `dev`:**
   - **Built-in programs by name.** A RUN of a name of one part without `.` starts the built-in program of that name (`wifi`, `netcheck`, `uptime`).
   - **Grants lent on, as the shell lends them.** A program that asks (`mind::request!`) gets the station's client, the system monitor's, and a network grant from the broker, bound to its PID.
   - **REBOOT,** a new control request, restarts the machine after its answer.
   - **Logging.** Each grant lent is logged with the start, and so is each REBOOT (MC-10.2).
   - **Without `dev`,** nothing changes.
4. **`mindlink reboot`** and `effector485`'s `reboot` kind.

## Acceptance criteria

- [ ] **The `link` suite in QEMU with `dev`:**
  - `memtest` by name;
  - `netcheck ping:10.0.2.2` answered;
  - `wifi status` from the station;
  - `uptime` from the system monitor;
  - a REBOOT that restarts the machine.

  Without the grants, each part says SKIP and names the request.
- [ ] **On the MacBook Pro over RS-485,** from the development machine with no one at the keyboard: `wifi join CoolNet2`, `netcheck ping` of the gateway, and a REBOOT.

Evidence for the configurations named only (MC-12.1). The mode widens what the cable reaches; it is not for machines others can plug into (MC-12.3).

## Progress

- **2026-10-21, written.**
  - **`linkproto`:** REBOOT (3), with a host test that a repeated REBOOT restarts once.
  - **`link`:** the `dev` word, built-in programs by name, lending, REBOOT.
  - **`mindlink` and `effector485`:** `reboot`.
  - **The QEMU suite's part,** which says SKIP without the grants.
  - **The maintainer's script:** `--link-dev`.
  - The grants were asked of `KRN` and the policy of `NET`.
- **2026-10-21, QEMU** (x86, 4 CPUs, a local run):
  - **With the four grants added to `init`'s `link` arm in the working tree only** (not committed; `KRN`'s to make):
    - the suite started `memtest` by name;
    - `netcheck ping:10.0.2.2` was answered through a network grant `link` lent, bound by the policy broker (`netpolicy.txt`: `netcheck 10.0.2.2 icmp`);
    - `wifi status` reached the station's client (no radio in that machine: "the station does not answer");
    - `uptime` got the system monitor;
    - REBOOT restarted the machine (QEMU exited, `-no-reboot`).
  - **Without them,** the suite started `memtest` by name, had REBOOT refused with UNSUPPORTED, and said SKIP for the rest.
  - The VM has a network card now, for `netcheck`.

## Related

[800-LNK-0009](800-LNK-0009-programs-started-for-the-master.md), [800-LNK-0011](800-LNK-0011-effector485.md), [550-DRV-0038](550-DRV-0038-retries-at-a-lower-rate.md), [docs/link/protocol.md](../docs/link/protocol.md).
