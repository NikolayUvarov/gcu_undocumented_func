# 550-DRV-0088 — A machine's owner adds a rule beside the regulatory catalogue: the Ryzen's channel 100 under UZ

**Type:** driver and catalogue (`regdb`, `rtw89_wifi`; the station's side asked of NET) · **Owner:** `DRV` · **Priority:** P3 · **Status:** later: the hardening stage (the maintainer, 2026-11-25) · **Blocked by:** — · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.4, MC-12.5

## Put off to the hardening stage (2026-11-25)

The maintainer, on what is left of the Ryzen: «на этапе тестирования мы не привязываемся к странам, первый приоритет - чтобы работало, hardening - это отдельный этап».

- **The bench's join on channel 100 does not wait for this task.** It goes under `00`, the rules every country allows, where the channel is listen-only: [213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md)'s section of the same date.
- **Nothing below is built.** The plan stands for the hardening stage, with NET's word still to come then.

## Problem

The maintainer, 2026-11-21, for the Ryzen's bench: «у нас правила по UZ должны работать и плюс канал 100 тоже должен работать». The bench's access point has its 5 GHz network on channel 100 (5500 MHz).

**What the catalogue says for UZ** (`regdb/db.txt`, wireless-regdb `master-2026-09-03`):

```
country UZ: DFS-ETSI
	(2402 - 2482 @ 40), (20)
	(5170 - 5250 @ 80), (20), AUTO-BW
	(5250 - 5330 @ 80), (20), DFS, AUTO-BW
```

- Channel 100 is in none of these rules, so the station bars it and the radio leaves it out.
- Upstream's `db.txt` read on 2026-11-21 has the same three rules for UZ. A newer snapshot would change nothing.

**What the catalogue is:** an upstream snapshot, "rather than an independently established current legal opinion" (`regdb/README.md`). The one who answers for where and how a machine transmits is its owner. Today the owner can choose the country and nothing else.

**What must not happen:** the catalogue's file edited by hand, or its rules quietly widened in code. A requirement is not reinterpreted to make a run pass (MC-12.5). An owner's rule is a statement of its own, kept apart, named as the owner's wherever it acts.

## Plan

1. **The shape of an owner's rule,** agreed with NET: a country, a frequency range, a width, a power and flags, as one of `db.txt`'s rule lines has them.
   - It is kept on the machine, with who set it and when.
   - It is set at the machine, with a confirmation at the keyboard as a network policy line has. Whether it may ever come over the network is NET's and the maintainer's to say; DRV's proposal is no.
   - It is named in the log at every start and in `wifi status`, as the owner's and not the catalogue's.
2. **`regdb`:** a country's plan made from the catalogue's rules and the owner's beside them.
   - The catalogue's file and its hash check stay as they are.
   - An owner's rule adds frequencies that no rule of that country covers. Where it overlaps a catalogue rule, the catalogue's power and flags stand: an owner's rule never weakens the catalogue's.
3. **To the radio.** A radio checks the limits it is handed against its own copy of the catalogue, and leaves out a channel outside the rules. So the owner's rules must reach it too. DRV's proposal to NET: `wlanhw.wit` 2.1, a call that hands the owner's rules before `set-country`; a 2.0 radio does not have the call and keeps to the catalogue.
4. **`rtw89_wifi`** takes them, logs them, and applies them as it does a catalogue rule, under its own limits (board, calibration, platform).
5. **The bench's first rule:** `UZ (5470 - 5725 @ 80), (20), DFS`. These values are DRV's most careful reading, not a fact about Uzbekistan's rules: the power of UZ's other 5 GHz rules, and the radar flag, so that the station only listens there until it hears the access point. The maintainer confirms or changes them as he enters the rule.

## Acceptance criteria

- [ ] Host cases: an owner's rule adds a range; it cannot raise a catalogue rule's power or drop its flags; with no owner's rule every plan is what it is today; `db.txt`'s hash is unchanged.
- [ ] A radio of `wlanhw` 2.0 and a station that knows no owner's rules behave as today.
- [ ] On the Ryzen, country UZ with the owner's rule: the scan lists channel 100 as listen-only, the log and `wifi status` name the rule as the owner's, and the station joins the access point there after its beacon ([213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md)'s run). Without the rule, channel 100 is barred as today. This holds for that machine and that access point (MC-12.1).

## Related

[213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md) (5 GHz in the driver; its join waits for this), [550-DRV-0049](../issues-done/550-DRV-0049-the-regulatory-catalogue-as-a-crate.done) (the catalogue as a crate), [550-DRV-0089](550-DRV-0089-a-machine-said-to-be-indoors.md) (the owner says the machine is indoors).
