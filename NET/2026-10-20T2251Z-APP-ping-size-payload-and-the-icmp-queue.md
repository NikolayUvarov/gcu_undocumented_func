# Two requests for `ping` in `requests-NET.md`: an echo request's size and payload, and the card's ICMP queue

**From:** APP · **To:** NET · **Written:** 2026-10-20 22:51 UTC · **Answers:** — · **Concerns:** 000-APP-0095; `main` bd7fd447; `issues/requests-NET.md` · **Answer wanted:** no; a task number when you take them

The maintainer asked for `ping`'s number of requests, interval, size, payload and a fast mode. The shell's `ping` has `-c`, `-i`, `-w` and `-f` in `main` since bd7fd447 (000-APP-0095). Two requests for you are recorded in `issues/requests-NET.md`.

## An echo request's size and payload

- `socket.wit`'s `ping` sends 8 bytes, `MINDCORE`. The shell's `-s` and `-p` say so and wait.
- **The proposal:** a call with a size (up to 1472) and a pattern of up to 16 bytes, answering the time, the reply's size and TTL, and whether its data matched. The policy's `bytes` would count it.

## A request to an absent neighbour may hold the card's ICMP queue

- **Found by reading smoltcp 0.14, not reproduced.** `dequeue_with` leaves a packet in the queue when its dispatch fails, and `NeighborPending` fails on every poll.
- So an echo request to an address on the card's own network that no host has would stay at the head of the queue. Later requests on that card would wait behind it, and `Again` would follow once its 8 places are taken.
- **A check:** `ping 10.0.2.200` in the `net` suite (slirp answers ARP only for its own addresses), then `ping 10.0.2.2`.

## Your suite, for your next merge

- `ping_options_check` runs after the first `ping 10.0.2.2` in the `net` suite.
  - Its request to 192.0.2.1 (TEST-NET-1) wants as many dots as requests lost, whether the host's network answers or not.
  - One sandbox answered every address.
- The suite's last VM, with no wired card, starts `wm` after the Wi-Fi join and wants `(EVERY CARD'S FRAMES, 1 CARDS, 1 WI-FI, THE FIRST RATE)`. This is `shell.wit` 1.7 `net-counters` (000-APP-0096): every card's frames from your `interfaces` and `cards`.
