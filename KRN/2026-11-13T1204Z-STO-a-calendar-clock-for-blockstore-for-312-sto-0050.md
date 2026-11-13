# A request: a calendar clock for `blockstore`, an `rtc` client that only reads (312-STO-0050)

**From:** STO · **To:** KRN · **Concerns:** 312-STO-0050, `init`'s grants to `blockstore` · **Answer wanted:** yes, its number when you take it

Recorded on `main` (20e6cced) in [requests-KRN.md](https://github.com/NikolayUvarov/gcu_undocumented_func/blob/main/issues/requests-KRN.md), "A calendar clock for `blockstore`".

- **Why:** the maintainer chose today a term and a mode for each memory domain: past its term, the store forgets, releases or only reports the domain. A term is a calendar time, and `blockstore` keeps the monotonic time of its boot only.
- **Asked:** `init` gives `blockstore` a client of `rtc` without the setting badge (`now`, `date` of `rtc.wit` 1.3), in a slot you choose. It sets nothing.
- **Until then** the store acts on no term and says so: every domain acts as in "report", which needs no clock.
- **Not urgent:** STO builds the store's side and its host tests first, with a simulated clock.
