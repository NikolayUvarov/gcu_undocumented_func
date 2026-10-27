# 211-DRV-0041 — `rtl8168` drives the DEXP's two RTL8168H ports

**Type:** driver · **Owner:** `DRV` · **Priority:** P1 · **Status:** in progress (2026-10-27) · **Blocked by:** `KRN`: a second `rtl8168` for the second port (asked on `internal-dev-messages`, 2026-10-27) · **Main task:** [211](211-intel-pc-from-a-sata-ssd.md), [550](550-network-on-real-hardware.md) · **Constitution:** MC-10.2, MC-12.1

## Problem

The maintainer's DEXP desktop has two wired ports, PCI 01:00.0 and 04:00.0, both `10ec:8168`. Its Wi-Fi is not driven yet either. The maintainer wants both ports working, so the server's Effector agent can reach its server over a cable.

- **The chip.** `rtl8168` read XID `0x541` and stopped with `INITIALIZATION FAILED: UnknownXid(1345)`. It accepted only the Ryzen's RTL8168G, XID `0x4c0` ([213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md)).
- **The second port.** `init` starts one `rtl8168`, for the first `10ec:8168` it finds.

## What was found

- **The XID.** `0x541` is the RTL8168H:
  - Linux v6.12 maps it to `RTL_GIGA_MAC_VER_46`, "RTL8168h/8111h";
  - FreeBSD's `if_re.c` at `16a79708`, the source `rtl8168` adapts, masks it to `RL_HWREV_8168H` (`0x54000000`).
- **The same flags as the 8168G.** FreeBSD gives `RL_HWREV_8168H` on device `0x8168` exactly the flags it gives `RL_HWREV_8168G`:
  - `PHYWAKE`, `PAR`, `DESCV2`, `MACSTAT`, `CMDSTOP`, `AUTOPAD`, `JUMBOV2`;
  - `CMDSTOP_WAIT_TXQ`, `WOL_MANLINK`, `8168G_PLUS`.

  No other branch of that driver tells the two apart, so the sequence `rtl8168` adapts serves both.
- **The PHY** reads `0x001cc800`, the same as on the Ryzen.

## Plan

1. **`rtl8168` accepts XID `0x541`** beside `0x4c0` (`controller::XIDS`; done).
   - On the DEXP, put in with `replace` over the link (2026-10-27): `XID=541 PHY=001CC800 LINK=DOWN MODE=MSI RINGS=256`.
   - `netstack` then waits for DHCP on card 0. No cable was in that port yet.
2. **The cable:** link up, DHCP, a ping of the gateway, the Effector agent connected to its server.
3. **The second port:** `KRN` is asked for a second `rtl8168` instance (`rtl8168#1`) on the second `10ec:8168`, as `virtio_net#1` is for a second VirtIO card.

## Acceptance criteria

- [ ] On the DEXP, port 01:00.0 with a cable: `LINK=UP`, an address by DHCP, the gateway answers a ping, the Effector agent reaches its server.
- [ ] The second port the same, once `init` starts its driver.
- [ ] `hwdocs/rtl8168.md` names both chips and what was run on each. Evidence for the DEXP only (MC-12.1).

## Related

[213-DRV-0031](213-DRV-0031-rtl8168-wired-network.md), [213-DRV-0030](../issues-done/213-DRV-0030-rtl8168-hardware-identification.done), [211-DRV-0040](../issues-done/211-DRV-0040-more-scratchpad-pages.done).
