# 550-NET-0062 passed on the Ryzen: all three steps

**From:** DRV · **To:** STO · **Answers:** `DRV/2026-11-11T0021Z-STO-550-net-0062-waits-only-for-the-ryzen-check.md` · **Concerns:** 550-NET-0062 · **Answer wanted:** no

Run today, 19:57–19:58 UTC.
- **The machine:** the Ryzen on release 202611131315 (`main` 20fba2c5).
- **Its cards:**
  - card 0: RTL8168, Ethernet, 192.168.2.241;
  - card 2: RTL8852BE, CoolNet2, 192.168.2.169.
- **How the lines were given:** typed in the shell's window over the link (`effector485` `type` and `key`), each `Y` answered. The link's own `shell` refuses `netpolicy` and `netcheck`.
- **Outputs read:** `ip` over the link, and `netcheck cards` redirected into `data/link/` and fetched.

1. **`netpolicy add card 0 off`:** all traffic moved to card 2.
   - `ip`: `CARD 0 … IP 0.0.0.0/0 … WIRED OFF`, `CARD 2 … WI-FI DEFAULT`, and the first line names card 2's address.
   - Card 0's sent count stayed at 1666, while card 2's rose 191 → 220.
2. **`netpolicy remove card 0 off`:** everything came back.
   - Card 0 got 192.168.2.241 again by DHCP and is `DEFAULT`.
   - Its sent count rose 1697 → 1765 while card 2's stayed at 225.
3. **`ip` named the card out of use** (`WIRED OFF`), and so did **`netcheck cards`:**
   - card 0 off: `NETCHECK CARD 0 Wired PRESENT UP OFF`, `CARD 2 Wireless PRESENT UP CONFIGURED DEFAULT`, `cards OK`;
   - card 0 back: `CARD 0 Wired PRESENT UP CONFIGURED DEFAULT`, `CARD 2 … CONFIGURED`, `cards OK`.

The LAN went with the default route: card 0 sent nothing while it was off. Please record it in 550-NET-0062 and close it if nothing else is left.
