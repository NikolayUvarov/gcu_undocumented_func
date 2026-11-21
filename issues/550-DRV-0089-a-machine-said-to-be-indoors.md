# 550-DRV-0089 — A machine its owner says is indoors may use the catalogue's indoor-only channels

**Type:** driver and catalogue (`regdb`, `rtw89_wifi`; the station's side asked of NET) · **Owner:** `DRV` · **Priority:** P3 (the maintainer, 2026-11-21: «это не главное сейчас») · **Status:** open (2026-11-21) · **Blocked by:** [550-DRV-0088](550-DRV-0088-the-owners-rule-beside-the-catalogue.md) (where an owner's statement is kept and how it reaches the radio) · **Main task:** [550](550-network-on-real-hardware.md) · **Constitution:** MC-6.1, MC-12.1, MC-12.5

## Problem

A rule of the catalogue marked `NO-OUTDOOR` or `NO-INDOOR` holds for one environment. The station does not know whether its machine is indoors, so it bars every such channel (`dot11`'s `one_environment`), and `rtw89_wifi` leaves them out too.

For some countries that is the whole band: under RU every 5 GHz rule is `NO-OUTDOOR`, so a desktop PC that never leaves its room has no 5 GHz channel at all (seen on the Ryzen, 2026-11-21, while the access points around it announced RU).

## Plan

1. **The owner's statement "this machine is indoors",** kept and shown as an owner's rule is ([550-DRV-0088](550-DRV-0088-the-owners-rule-beside-the-catalogue.md)): set at the machine, named in the log and in `wifi status`.
2. **With it,** a `NO-OUTDOOR` rule is used as the catalogue gives it, and a `NO-INDOOR` rule stays barred. Without it, both stay barred as today.
3. **A machine that moves** (a laptop, a phone) is the hard case: the statement is its owner's, and it holds until he takes it back. Say so where it is set.
4. The station's side is NET's; the radio's side is `rtw89_wifi`'s and any other radio's that reads the catalogue.

## Acceptance criteria

- [ ] Host cases: with the statement, RU's 5 GHz channels are planned with their limits; without it, none; a `NO-INDOOR` rule is barred either way.
- [ ] On a machine under such a country the scan and the join use an indoor-only channel, and the log names the owner's statement.

## Related

[550-DRV-0088](550-DRV-0088-the-owners-rule-beside-the-catalogue.md), [213-DRV-0087](213-DRV-0087-rtl8852be-on-5-ghz.md).
