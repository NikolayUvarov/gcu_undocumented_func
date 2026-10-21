# 800-KRN-0142 — The link's network policy client, badged to launch only

**Type:** `init` · **Owner:** kernel session · **Priority:** P2 · **Status:** open (2026-10-21) · **Blocked by:** [800-NET-0038](800-NET-0038-a-policy-client-that-may-only-launch.md) (`BADGE_LAUNCH` and the broker's refusals in `main`) · **Main task:** — (for [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md)) · **Constitution:** MC-3.4, MC-11.3, MC-11.6

From the network track's request in [requests-KRN.md](requests-KRN.md) (2026-10-21), numbered here.

## Problem

- **What `link` holds.** In developer mode (`dev` in `link.conf`), `init` lends `link` the network policy broker's client as the shell holds it: `self.lend(&mut grants, SLOT_NETPOLICY, "netpolicy")` (800-KRN-0139).
- **What that client may do.** Besides asking flow grants for the programs `link` starts, it may add and remove policy lines and revoke other programs' grants.
- **Why that is too much.** The shell does those only after the user agrees (108). `link` has no user to ask.
- **What `link` uses.** It calls `prepare`, `take` and `bind` only (LNK to NET, 2026-10-21).

## Plan

- In developer mode, `init` mints `link`'s `SLOT_NETPOLICY` client from `netpolicy`'s keeper with `mind::network::BADGE_LAUNCH`, as it mints the updater's lifecycle client with `BADGE_REBOOT`.
- The developer mode's boot line names the badge.
- The `link` suite's check of `link`'s grants expects the badge. That suite is the link track's: the change goes to LNK as a request if the check does not read badges already.

## Acceptance criteria

- [ ] In QEMU (x86), `link` in developer mode holds `SLOT_NETPOLICY` badged `BADGE_LAUNCH` (`stat caps`), and starts `netcheck` with a flow grant the policy names.
- [ ] A policy change through that client is refused and logged by the broker; the policy is unchanged.

## Related

[800-NET-0038](800-NET-0038-a-policy-client-that-may-only-launch.md), [800-KRN-0139](800-KRN-0139-the-links-developer-mode.md), [800-LNK-0012](../issues-done/800-LNK-0012-the-developer-mode.done).
