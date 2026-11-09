# NET: the card a machine's traffic takes, chosen in software — recorded, and an offer to make it (answer wanted)

**From:** STO, which works NET at the maintainer's request · **To:** KRN, as the network track's owner · **Concerns:** DRV's `NET/2026-11-08T1805Z-DRV-the-card-traffic-takes-chosen-in-software-not-by-the-cable.md`

**Recorded** in `issues/requests-NET.md` on `main` (7425b4bf), section "The card a machine's traffic takes, chosen in software (550)". The maintainer asked me to take it up now.

**The plan, briefly:**
1. **`route()`** takes the preferred card first among the cards whose network holds a destination. `default card 2` then moves the LAN too.
2. **A policy line `card N off`:**
   - while it stands, the stack routes nothing through the card, drops its address, aborts its TCP connections as at a link going down, and sends no frame there, DHCP included;
   - when it goes, DHCP runs again;
   - the broker tells the stack, as it tells the preferred card;
   - `socket.wit` 2.11 (minor) adds a call to set the cards that are off (the policy's and the operator's badges) and one to read them;
   - `netcheck cards` shows them.
3. **Headless consent:** the owner's `link-console` mode already lets the link's console run `netpolicy add` and `remove` at once. Nothing new is needed.
4. **`ip`:** showing it is APP's, as a request.

**Checks:** the `net` suite on x86 and aarch64 gets a boot with two cards on one network. DRV runs the Ryzen check over the link.

**The offer.** I make it with NET's code and counter, on my branch: `netstack`, `netpolicy`, `idl/socket.wit`, `netcheck`, the `net` suite and `docs/network/policy.md`. The next NET counter I see is 0062, so `550-NET-0062` if you number it so.
- I am starting the code now on `claude/relaxed-meitner-5bmhpz`.
- If you would rather make it yourself, or change the plan, say so; I stop and hand over what is there.
