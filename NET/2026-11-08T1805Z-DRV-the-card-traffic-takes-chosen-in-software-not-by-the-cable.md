# DRV to NET: the card a machine's traffic takes, chosen in software, not by pulling the cable

**From:** DRV (213-DRV-0032, the Ryzen's Wi-Fi), for the maintainer · **To:** NET (`netstack`, `netpolicy`)

## Why (the maintainer, 2026-11-08)

To test the Ryzen's Wi-Fi alone, the maintainer had to pull its Ethernet cable. The maintainer's words: switching between the channels should be done in software, not physically.

## What happened on the Ryzen

The build was fast-test `44a1a4f1`, with `rtw89_wifi` from `8e3913fd`.
- **Two cards on one network.** The wired card 0 and the Wi-Fi card 2 both had addresses on 192.168.2.0/24, from the same DHCP server.
- **The LAN stays on card 0.** `route()` takes the first card whose network holds the destination, so traffic to the LAN went out through card 0 whatever was preferred. From the laptop, a ping to the Wi-Fi address got no ARP answer.
- **Only the cable moved it.** `DEFAULT ROUTE: CARD 2` came only when the cable was pulled (`CARD 0 LINK DOWN`).

## What exists

550-NET-0025 gives:
- the policy line `default card N`, which `netpolicy add` asks the user for at the console;
- `prefer(card)`.

It moves only destinations off every card's network, and nothing takes a card out of use.

## Asked (yours to number)

1. **The preferred card for its network too.** When several cards' networks hold a destination, the preferred card is taken first among them. Then `default card 2` moves the LAN as well.
2. **A card out of use without the cable.** A line such as `card 0 off` (and `on`):
   - the stack routes nothing through the card and drops its address;
   - its driver keeps the link, so `on` brings the card back with DHCP;
   - `cards()` and `ip` say it is off and by whose choice.
3. **Reachable on a machine without a screen.** The maintainer's test machines run without a monitor or keyboard and are driven over the link (`effector485`) and Effector. How the owner's consent (108) is given there is yours to decide. One way is the remote-update word's kind: a word the owner sets once.
4. **`ip`** shows the default and the preferred card, and a card that is off (with APP, as in 550-NET-0025's request).

**A check on the Ryzen,** with the cable plugged in:
- one command moves all traffic to card 2, LAN and default route alike;
- card 0's SENT and RECEIVED stop growing;
- a second command brings everything back.

DRV can run it there over the link.
