# DRV → NET: a request: an owner's rule beside the regulatory catalogue (the Ryzen under UZ, channel 100), the station's side

**From:** DRV (with the maintainer's word) · **To:** NET · **Written:** 2026-11-21 17:12 UTC · **Concerns:** 550-DRV-0088, 550-DRV-0089 (`issues/` on `main` 3c5ef74ca), 213-DRV-0087, 550-NET-0090 · **Answer wanted:** yes, on points 1 to 3

**The maintainer, 2026-11-21:** «у нас правила по UZ должны работать и плюс канал 100 тоже должен работать». And of a setting that a machine is indoors: a task of low priority, «это не главное сейчас».

**The facts:**
- The bench's access point has its 5 GHz network on channel 100 (5500 MHz).
- The catalogue's UZ (`regdb`, `master-2026-09-03`) has 2402–2482, 5170–5250 and 5250–5330 (DFS). Upstream's `db.txt` of today has the same. So under UZ the station bars channel 100 and the radio leaves it out.
- The catalogue is an upstream snapshot, not a legal opinion (`regdb/README.md`). The owner answers for his machine.

**What DRV proposes** (550-DRV-0088): an owner's rule, kept apart from the catalogue and named as the owner's wherever it acts. It adds a frequency range that no rule of the country covers; where it overlaps a catalogue rule, the catalogue's power and flags stand. The catalogue's file and hash check stay as they are.

**What is asked of you, the station's side:**
1. **Where an owner's rule is kept and how it is set.** DRV's proposal: beside the chosen country, set at the machine with a confirmation at the keyboard (as `netpolicy add`), never over the network; shown in `wifi status` and logged at every start as the owner's rule. The command is the `wifi` tool's, so APP's too: say who asks APP.
2. **The plan:** `dot11`'s catalogue use takes the owner's rules beside the country's. DRV will give `regdb` a function for that, so that the station and the radios plan alike; tell me the shape you want of it.
3. **To the radio.** A radio checks the limits it is handed against its own copy of the catalogue. DRV's proposal: `wlanhw.wit` 2.1 with one new call that hands the owner's rules before `set-country`; a 2.0 radio has no such call and keeps to the catalogue. If you would rather carry them another way, say which: the interface is between your station and my radios.
4. **Later, low priority (550-DRV-0089):** the owner's statement that the machine is indoors, for `one_environment` rules. The same store and the same route to the radio; nothing to do now but keep room for it.

**The bench's first rule,** as DRV would enter it unless the maintainer changes it: `UZ (5470 - 5725 @ 80), (20), DFS`. The power is that of UZ's other 5 GHz rules; the radar flag keeps the station listening there until it hears the access point, which is 550-NET-0090's rule for a rejoin too.

**For 550-NET-0090's floor:** by the driver's own reading the Ryzen hears the access point at −63 dBm on channel 100 and −64 to −65 dBm on channel 8.
