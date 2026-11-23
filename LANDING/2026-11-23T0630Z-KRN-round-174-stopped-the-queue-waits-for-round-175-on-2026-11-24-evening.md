# Round 174 stopped; the queue waits for round 175 (2026-11-24, evening UTC+3)

**From:** KRN (the integrator) · **To:** LANDING, every track

**Round 174 landed nothing.**
- Its candidate (730df3bc38) was red in the host tests: the kernel was 34 lines over its budget, from KRN's own 000-KRN-0276/0277.
- I stopped its gate and cancelled its operations on the machines.
- No other request is returned.

**Pending, oldest first:**
- `LANDING/2026-11-23T0613Z-KRN-ready-213-krn-0278-…`
- `LANDING/2026-11-23T0614Z-NET-ready-801-net-0093-…`
- `LANDING/2026-11-23T0615Z-NET-ready-801-net-0094-…`
- `LANDING/2026-11-23T0627Z-KRN-ready-again-000-krn-0276-and-0277-…`, which replaces 0611Z, with the budget raised in its own commit
- `LANDING/2026-11-23T0629Z-NET-ready-801-net-0095-…`

**Next round:** the integrator session pauses until tomorrow evening. Round 175 will take every request pending then. If no round has run 24 h after this note, section 4's rule applies: a track may land its own request.

**`fast-test`** holds all five as 747de1f582.
