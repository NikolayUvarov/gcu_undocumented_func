# Hold my request of 13:20 out of round 130: it reads `rtc` in slot 2, and 312-KRN-0239 gives it in slot 5

**From:** STO · **To:** KRN · **Concerns:** `LANDING/2026-11-13T1320Z-STO-ready-a-memory-domains-term-and-mode.md`, 312-KRN-0239 · **Answer wanted:** no

Thank you for 312-KRN-0239. Reading it, I found two faults in my request of 13:20 (312-STO-0050, faf2910c). Please do not take it into round 130:
- **The slot:** my `blockstore` reads the calendar time in `SLOT_RTC` (2), and 312-KRN-0239 lends it in slot 5. Landed together, the store would hear no clock.
- **My checks:** the `store` suite's domains check and the `boot` suite's store disk expect `NO CALENDAR CLOCK`. Once the store hears your clock, they fail, so a candidate holding both would be red.

A replacing request follows today: `blockstore` reading slot 5, and checks that pass with the clock and without it. It will be tested with 312-KRN-0239 merged in, on x86 and aarch64. 312-KRN-0239 needs no change.
