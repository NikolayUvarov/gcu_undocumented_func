# 800-KRN-0139 — The link's developer mode: with `dev` in `link.conf`, `init` lends `link` what the shell lends its programs

**Type:** `init` · **Owner:** kernel session · **Priority:** P1 · **Status:** made, in `fast-test` (2026-10-21); the gate is left · **Blocked by:** — · **Main task:** [800](800-mind-link.md) (the link track's) · **Constitution:** MC-2.3, MC-3.11, MC-10.2

## Problem

The maintainer asked on 2026-10-21: "Can we add debug developer mode to allow full normal debug using this effector link? Add necessary words to link.conf to be able do the tests."
- **The link track's request** (`internal-dev-messages`, `KRN/2026-10-21T0600Z-LNK-a-developer-mode-for-the-link-the-word-dev.md`), for 800-LNK-0012: the word `dev` in `link.conf`, and four grants with it.
- **What it is for:** the agent at the other end of the cable tests the MacBook Pro's Wi-Fi by itself, with no one at the keyboard: it joins the network, pings and restarts the machine.
- **Why it cannot now:** a program `link` starts gets an application's standard grants and nothing more. It cannot reach the network or the station.

## Plan

**With `dev` among `link.conf`'s words,** `init`'s `link` arm adds four grants. These are the clients the shell lends its programs on request, without the shell's question on the screen:
- **`sysmon`'s client** in `SLOT_SYSINFO` (10), lent on for `REQUEST_SYSINFO`;
- **the network policy broker's client** in `SLOT_NETPOLICY` (19), as the shell's;
  - `link` asks a flow grant for each program that requests `REQUEST_NETWORK`;
  - the policy decides as for the shell. NET answers whether it wants a rule or a badge for these (`NET/2026-10-21T0600Z-LNK-flow-grants-for-programs-the-link-starts.md`);
- **the Wi-Fi station's configuration client** with `BADGE_WIFI_CHANGE` in `SLOT_WIFI` (33), lent on for `REQUEST_WIFI`;
- **`init`'s lifecycle client badged `BADGE_REBOOT`** in `SLOT_LIFECYCLE` (11), as the updater's. It may ask `reboot` and nothing else of `init`.

**The servers start after `link`.** `init` makes each keeper at its first use and keeps it across restarts, so these clients reach their servers once they run. On a machine without Wi-Fi the station's client finds no server (`ERR_PEER`).

**Without `dev`, nothing changes:** `link` holds its four grants of 800-KRN-0128 and 0132, and its endpoint quota of 0137.

**What it means for security.** Whoever holds the cable may debug the machine as its user would. The cable is not authenticated (`docs/link/protocol.md`, section 7).
- The word is for the maintainer's own machines.
- `docs/profile/bootstrap.md` says so in `link`'s row.

## Acceptance criteria

- [x] With `dev` in `link.conf`, `stat caps` of `link` shows the four grants of 800-KRN-0128 and 0132 and these four, and nothing else granted (a one-off local run of the `link` suite with `run dev`, 2026-10-21):
  - 10 → `sysmon`;
  - 11 → `init`;
  - 19 → `netpolicy`;
  - 33 → the station's keeper, no server in QEMU.
- [ ] The link track's `link` suite checks them with `dev`, and their absence without it (800-LNK-0012).
- [ ] The gate passes.

## Related

[800-KRN-0128](../issues-done/800-KRN-0128-init-starts-usb-serial-and-link.done), [800-KRN-0132](800-KRN-0132-link-starts-programs-for-the-master.md), [800-KRN-0137](800-KRN-0137-link-may-make-one-endpoint.md), [800-KRN-0138](800-KRN-0138-stop-and-watch-the-programs-a-caller-started.md), 800-LNK-0012, [docs/profile/bootstrap.md](../docs/profile/bootstrap.md).
