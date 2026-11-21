# 550-DRV-0088 — A machine's owner adds a rule beside the regulatory catalogue: the Ryzen's channel 100 under UZ

**Type:** driver and catalogue (`regdb`, `rtw89_wifi`; the station's side asked of NET) · **Owner:** `DRV` · **Priority:** P1 · **Status:** open (2026-11-21) · **Blocked by:** NET's word on how an owner's rule reaches the station and the radio · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.4, MC-12.5

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

- [x] Host cases: an owner's rule adds a range; it cannot raise a catalogue rule's power or drop its flags; with no owner's rule every plan is what it is today; `db.txt`'s hash is unchanged (`regdb`'s plan, 2026-11-21; the station and the radio do not use it yet).
- [ ] A radio of `wlanhw` 2.0 and a station that knows no owner's rules behave as today.
- [ ] On the Ryzen, country UZ with the owner's rule: the scan lists channel 100 as listen-only, the log and `wifi status` name the rule as the owner's, and the station joins the access point there after its beacon ([213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md)'s run). Without the rule, channel 100 is barred as today. This holds for that machine and that access point (MC-12.1).

## Progress

**2026-11-21, `regdb`'s plan with the owner's rules beside the catalogue** (plan point 2; host cases only; NET's answer on the station's side is not in yet, so nothing reaches the station or the radio):
- **`OwnerRule::new(country, start_khz, end_khz, max_bandwidth_khz, max_eirp_mbm, flags)`** holds the fields of one of `db.txt`'s rule lines for one country, kept apart from the catalogue. It is checked as a catalogue line's fields are:
  - a country the catalogue holds, not `00` (an owner's rule never applies to the world rules);
  - a range at least the width;
  - a positive width and power;
  - `db.txt`'s flags alone (`Flags::with` combines them).

  Making one grants nothing. Who set it and when is the keeper's to record (plan point 1, NET's).
- **`Country::plan(owner, center_khz, width_khz) -> Result<Planned, Error>`** gives the one rule that holds the whole channel, and whose it is (`Source::Catalogue { source_line }` or `Source::Owner { index }`), in any band:
  - **The catalogue first,** as `channel_2g20` finds a rule (two holding it: `AmbiguousRule`).
  - **An owner's rule for this country** can hold the channel only where no catalogue rule of the country touches it. So a catalogue rule's power and flags stand wherever it speaks, and its edge is not widened: a channel across a catalogue rule's edge stays outside the rules.
  - **Two owner's rules holding one channel** are refused as two catalogue rules are.
- **With no owner's rule,** every plan is the catalogue's as today.
- **The shape is a first one.** It takes one channel and a slice of the owner's rules, so either consumer can hand in what it keeps. If NET wants the rules as a list beside `source_rules` instead (as `dot11`'s `station_rules` reads the catalogue), it changes without touching the rule's checks.
- **Not decided here:** a ceiling on an owner's rule's power. None is set beyond the format's checks; that is the maintainer's word as he enters the rule.
- **Host cases** (`tests/regdb_host.rs`):
  - **Plans unchanged:** with no owner's rule, every country's plan on 2.4 GHz channels 1 to 14 is `channel_2g20`'s, rule, line and error alike. On the 28 5 GHz channels it is the one rule that holds the 20 MHz channel, as the radio's `country.rs` reads it.
  - **The world:** its plan takes no owner's rule.
  - **The bench's rule:** `UZ (5470 - 5725 @ 80), (20), DFS` gives channels 100 to 140 at 20 dBm with DFS, named as the owner's. 144 runs past 5725 MHz; 80 MHz fits it and 160 does not. RU's rule does nothing for UZ, and the index names the rule in the list.
  - **No weakening:** a UZ rule over 5150-5350 MHz at 30 dBm with no flags leaves 36 to 64 at the catalogue's 20 dBm, and 52 to 64 with its DFS. A 30 dBm rule over 2.4 GHz leaves 1 to 13 at 20 dBm, and channel 14 (across 2482 MHz) outside.
  - **Edges:** 5330-5350 and 5150-5170 MHz, which no catalogue rule touches, are the owner's; a 40 MHz channel across 5330 MHz and a 20 MHz one across 5170 MHz are not.
  - **The checks:** `00`, `ZZ`, `uz`, an empty or reversed range, a range narrower than its width, a zero width and a zero or negative power are refused.
  - **The catalogue untouched:** `db.txt`'s SHA-256 is still `SOURCE_SHA256`, and UZ's three rules are as before.
- `db.txt`, `src/tables.rs` and `scripts/regdb.py` are unchanged; `python3 scripts/regdb.py --check` passes.

## Where the radio consults its own copy of the catalogue (the places an owner's rule must reach; not changed yet)

- **`rtw89_wifi/src/country.rs`, `Limits::accept`,** is the only place the radio reads the catalogue:
  - `regulatory::select(code)`, or `regulatory::world()` for `00`;
  - then `rule(country, channel)` for each channel the station names. On 2.4 GHz that is `channel_2g20` and `assess_initial_driver(Environment::Unknown)`. On 5 GHz it is the one source rule that holds the 20 MHz channel: DFS or NO-IR make the channel listen-only, and WMM, NO-INDOOR, NO-OUTDOOR, NO-OFDM, PTP-ONLY or PTMP-ONLY leave it out.
  - The rule's EIRP caps the station's, and a channel with no rule is left out.
- **Its callers,** which an owner's rule reaches through it:
  - `receive_service.rs`, `set-country` before admission: `Limits::accept(code, settings, limits)`;
  - `receive_service.rs`, `world()` (no country before the first send, 801-DRV-0071): `Limits::world`, that is `accept("00", …)`, which takes no owner's rule;
  - `service.rs`, `country()` (a `set-country` in data mode): `Limits::accept`, which decides whether the admitted channel is kept.
- **Made from `Limits`, with no catalogue read of their own:** `info`'s channel list, `sends_on` and the listen-only rule, the tune's check that a channel is left (`SetChannel`), the admission's limit (`limits.applied(admit)`), the calibration's EIRP from it, and the data mode's check that its channel is kept.
- **The change when NET has answered** (plan points 3 and 4):
  - `Limits::accept` takes the owner's rules beside the code, and uses `Country::plan` for both bands in place of `rule()`'s own walk, the radio's own exclusions kept as they are;
  - `Applied` carries whose rule it is, so that `COUNTRY XX` and the admission's line name an owner's rule as the owner's;
  - `hwdocs/rtl8852be-regulatory.md`'s consumer contract says so.

## Related

[213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md) (5 GHz in the driver; its join waits for this), [550-DRV-0049](../issues-done/550-DRV-0049-the-regulatory-catalogue-as-a-crate.done) (the catalogue as a crate), [550-DRV-0089](550-DRV-0089-a-machine-said-to-be-indoors.md) (the owner says the machine is indoors).
