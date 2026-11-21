#![allow(dead_code)]
#[path = "../regdb/src/lib.rs"]
mod regulatory;

use regulatory::{Assessment, DfsRegion, Environment, Error, Flags, OwnerRule, Restriction, Source};
#[path = "../libmind/src/sha256.rs"]
mod sha256;

fn country(code: &[u8; 2]) -> &'static regulatory::Country {
    regulatory::select(*code).unwrap().country()
}

#[test]
fn all_source_entries_are_sorted_distinct_and_selectable_without_world_fallback() {
    let entries = regulatory::countries();
    assert_eq!(entries.len(), 182);
    assert!(entries.windows(2).all(|pair| pair[0].alpha2() < pair[1].alpha2()));
    assert_eq!(entries.iter().filter(|entry| entry.is_world()).count(), 1);
    for entry in entries.iter().filter(|entry| !entry.is_world()) {
        assert!(core::ptr::eq(country(&entry.alpha2()), entry));
        assert!(entry.channels_2g20().count() > 0);
    }
    assert_eq!(country(b"AN").alpha2(), *b"AN");
    assert!(matches!(regulatory::select(*b"00"), Err(Error::WorldIsNotCountry)));
    assert!(matches!(regulatory::select(*b"ZZ"), Err(Error::UnknownCountry)));
    for invalid in [*b"uz", *b"Uz", *b"U ", [0, 0], [255, 255]] {
        assert!(matches!(regulatory::select(invalid), Err(Error::CountryFormat)));
    }
}

#[test]
fn uz_covers_exact_twenty_mhz_edges_and_is_eirp_not_conducted_power() {
    let uz = country(b"UZ");
    assert_eq!(uz.dfs_region(), DfsRegion::Etsi);
    assert_eq!(uz.channels_2g20().map(|channel| channel.number).collect::<Vec<_>>(), (1..=13).collect::<Vec<_>>());
    for number in 1..=13 {
        let channel = uz.channel_2g20(number).unwrap();
        assert_eq!(channel.center_khz, 2_407_000 + number as u32 * 5000);
        assert_eq!(channel.rule.max_eirp_mbm, 2000);
        assert_eq!(channel.rule.eirp_half_dbm(), 40);
        assert_eq!(channel.rule.start_khz, 2_402_000);
        assert_eq!(channel.rule.end_khz, 2_482_000);
        assert!(channel.rule.flags.is_empty());
        assert_eq!(channel.assess_initial_driver(Environment::Unknown), Assessment::EligibleForIntersection);
    }
    assert_eq!(uz.channel_2g20(14), Err(Error::OutsideRules));
    assert_eq!(uz.channel_2g20(0), Err(Error::Channel));
    assert_eq!(uz.channel_2g20(255), Err(Error::Channel));
}

#[test]
fn independent_selection_changes_coverage_without_mutating_prior_choice() {
    let uz = regulatory::select(*b"UZ").unwrap();
    let us = regulatory::select(*b"US").unwrap();
    assert_eq!(uz.provenance(), regulatory::PROVENANCE);
    assert!(uz.country().channel_2g20(13).is_ok());
    assert!(us.country().channel_2g20(11).is_ok());
    assert_eq!(us.country().channel_2g20(12), Err(Error::OutsideRules));
    assert_eq!(us.country().channel_2g20(13), Err(Error::OutsideRules));
    assert!(uz.country().channel_2g20(13).is_ok());
}

#[test]
fn world_passive_edges_and_japan_fourteen_keep_restrictions() {
    let world = regulatory::world();
    assert_eq!(world.alpha2(), *b"00");
    assert!(!world.channel_2g20(11).unwrap().rule.flags.contains(Flags::NO_IR));
    for number in [12, 13] {
        let channel = world.channel_2g20(number).unwrap();
        assert!(channel.rule.flags.contains(Flags::NO_IR));
        assert!(channel.rule.flags.contains(Flags::AUTO_BW));
        assert_eq!(channel.assess_initial_driver(Environment::Unknown), Assessment::PassiveOnly);
    }
    let jp14 = country(b"JP").channel_2g20(14).unwrap();
    assert_eq!(jp14.center_khz, 2_484_000);
    assert!(jp14.rule.flags.contains(Flags::NO_OFDM));
    assert_eq!(jp14.assess_initial_driver(Environment::Indoor), Assessment::Unsupported(Restriction::DriverChannel));
}

#[test]
fn every_indoor_only_country_requires_explicit_indoor_context() {
    for code in [b"EG", b"ID", b"OM", b"QA", b"SY", b"UA"] {
        let channel = country(code).channel_2g20(1).unwrap();
        assert!(channel.rule.flags.contains(Flags::NO_OUTDOOR));
        for environment in [Environment::Unknown, Environment::Outdoor] {
            assert_eq!(channel.assess_initial_driver(environment), Assessment::Unsupported(Restriction::Environment));
        }
        assert_eq!(channel.assess_initial_driver(Environment::Indoor), Assessment::EligibleForIntersection);
    }
}

#[test]
fn actual_two_ghz_dfs_is_not_inferred_from_country_region_or_silently_allowed() {
    let tg = country(b"TG").channel_2g20(1).unwrap();
    assert!(tg.rule.flags.contains(Flags::DFS));
    assert_eq!(tg.assess_initial_driver(Environment::Indoor), Assessment::Unsupported(Restriction::Dfs));
    assert!(!country(b"UZ").channel_2g20(1).unwrap().rule.flags.contains(Flags::DFS));
}

#[test]
fn full_source_catalog_retains_no_indoor_and_wmm_in_unsupported_bands() {
    let occurrences: usize = regulatory::countries().iter().map(|country| country.source_rules().len()).sum();
    assert_eq!(occurrences, 1016);
    for code in [b"IR", b"OM"] {
        assert!(country(code).source_rules().any(|(rule, _)| rule.flags.contains(Flags::NO_INDOOR)));
    }
    let (wmm_rule, line) = country(b"AD").source_rules().find(|(rule, _)| rule.wmm.is_some()).unwrap();
    assert!(line > 0);
    let wmm = wmm_rule.wmm.unwrap();
    assert_eq!(wmm.name, "ETSI");
    assert_eq!((wmm.categories[0].cw_min, wmm.categories[0].cw_max, wmm.categories[0].aifsn, wmm.categories[0].cot_ms), (3, 7, 2, 2));
    assert_eq!(wmm.categories[6].cw_max, 63);
}

#[test]
fn integer_power_floor_never_wraps_larger_source_limits() {
    let us = country(b"US").channel_2g20(1).unwrap();
    assert_eq!(us.rule.max_eirp_mbm, 3000);
    let maximum = regulatory::countries().iter().flat_map(|country| country.channels_2g20())
        .map(|channel| channel.rule.eirp_quarter_dbm()).max().unwrap();
    assert_eq!(maximum, 144);
    let (floor, _) = country(b"AD").source_rules().find(|(rule, _)| rule.max_eirp_mbm == 2698).unwrap();
    assert_eq!(floor.eirp_half_dbm(), 53);
    assert_eq!(floor.eirp_quarter_dbm(), 107);
}

#[test]
fn every_enumerated_channel_fits_complete_bandwidth_and_has_source_identity() {
    for country in regulatory::countries() {
        assert!(country.source_line() > 0);
        for channel in country.channels_2g20() {
            assert_eq!(channel.country, country.alpha2());
            assert!(channel.rule.start_khz <= channel.center_khz - 10_000);
            assert!(channel.center_khz + 10_000 <= channel.rule.end_khz);
            assert!(channel.rule.max_bandwidth_khz >= 20_000);
            assert!(channel.source_line > country.source_line());
        }
    }
}

// 550-DRV-0088: an owner's rule beside the catalogue.
const FIVE_GHZ: [u8; 28] = [36, 40, 44, 48, 52, 56, 60, 64, 100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140, 144, 149, 153,
    157, 161, 165, 169, 173, 177];
fn center_khz(channel: u8) -> u32 {
    match channel { 14 => 2_484_000, 1..=13 => 2_407_000 + u32::from(channel) * 5000, _ => 5_000_000 + u32::from(channel) * 5000 }
}
// The bench's first rule as the issue proposes it, for the Ryzen's channel 100: `UZ (5470 - 5725 @ 80), (20), DFS`.
fn bench() -> OwnerRule { OwnerRule::new(*b"UZ", 5_470_000, 5_725_000, 80_000, 2000, Flags::DFS).unwrap() }

#[test]
fn with_no_owners_rule_every_plan_is_the_catalogues() {
    for country in regulatory::countries() {
        for number in 1..=14 {
            let planned = country.plan(&[], center_khz(number), 20_000);
            match country.channel_2g20(number) {
                Ok(channel) => {
                    let p = planned.unwrap();
                    assert_eq!((p.rule, p.source), (*channel.rule, Source::Catalogue { source_line: channel.source_line }));
                }
                Err(error) => assert_eq!(planned, Err(error)),
            }
        }
        // On 5 GHz, the one source rule that holds the whole 20 MHz channel, as the radio's country.rs reads it today.
        for number in FIVE_GHZ {
            let centre = center_khz(number);
            let holding: Vec<_> = country.source_rules().filter(|(r, _)| r.max_bandwidth_khz >= 20_000
                && r.start_khz + 10_000 <= centre && centre + 10_000 <= r.end_khz).collect();
            let planned = country.plan(&[], centre, 20_000);
            match holding.as_slice() {
                [] => assert_eq!(planned, Err(Error::OutsideRules)),
                [(rule, line)] => {
                    let p = planned.unwrap();
                    assert_eq!((p.rule, p.source, p.country), (**rule, Source::Catalogue { source_line: *line }, country.alpha2()));
                }
                _ => assert_eq!(planned, Err(Error::AmbiguousRule)),
            }
        }
    }
    // The world's plan takes no owner's rule (none can be made for 00), even where its own rules leave a gap.
    let world = regulatory::world();
    for centre in (5_150_000..=5_900_000).step_by(10_000) {
        assert_eq!(world.plan(&[bench()], centre, 20_000), world.plan(&[], centre, 20_000), "{centre} kHz");
    }
}

#[test]
fn an_owners_rule_adds_a_range_no_catalogue_rule_of_its_country_touches() {
    let uz = country(b"UZ");
    for number in [100, 104, 108, 112, 116, 120, 124, 128, 132, 136, 140] {
        assert_eq!(uz.plan(&[], center_khz(number), 20_000), Err(Error::OutsideRules), "channel {number} without the rule");
        let p = uz.plan(&[bench()], center_khz(number), 20_000).unwrap();
        assert_eq!(p.source, Source::Owner { index: 0 });
        assert_eq!((p.rule.max_eirp_mbm, p.rule.eirp_half_dbm(), p.rule.flags), (2000, 40, Flags::DFS));
        assert!(p.rule.wmm.is_none());
    }
    // 144 runs past 5725 MHz; 80 MHz fits it, 160 does not.
    assert_eq!(uz.plan(&[bench()], center_khz(144), 20_000), Err(Error::OutsideRules));
    assert_eq!(uz.plan(&[bench()], 5_530_000, 80_000).unwrap().source, Source::Owner { index: 0 });
    assert_eq!(uz.plan(&[bench()], 5_570_000, 160_000), Err(Error::OutsideRules));
    // Another country's rule does nothing here; the index names the rule in the list given.
    let ru = OwnerRule::new(*b"RU", 5_470_000, 5_725_000, 80_000, 2000, Flags::DFS).unwrap();
    assert_eq!(uz.plan(&[ru], center_khz(100), 20_000), Err(Error::OutsideRules));
    assert_eq!(uz.plan(&[ru, bench()], center_khz(100), 20_000).unwrap().source, Source::Owner { index: 1 });
    assert_eq!(country(b"RU").plan(&[ru, bench()], center_khz(100), 20_000).unwrap().source, Source::Owner { index: 0 });
    // Two of the owner's rules holding one channel are refused, as two catalogue rules are.
    assert_eq!(uz.plan(&[bench(), bench()], center_khz(100), 20_000), Err(Error::AmbiguousRule));
}

#[test]
fn an_owners_rule_cannot_raise_a_catalogue_rules_power_drop_its_flags_or_widen_its_edge() {
    let uz = country(b"UZ");
    // Over UZ's 5170-5330 MHz at 30 dBm with no flags: 36 to 64 keep the catalogue's 20 dBm, and 52 to 64 its DFS.
    let wide = OwnerRule::new(*b"UZ", 5_150_000, 5_350_000, 80_000, 3000, Flags::AUTO_BW).unwrap();
    for number in [36, 40, 44, 48, 52, 56, 60, 64] {
        let with = uz.plan(&[wide], center_khz(number), 20_000).unwrap();
        assert_eq!(with, uz.plan(&[], center_khz(number), 20_000).unwrap(), "channel {number}");
        assert!(matches!(with.source, Source::Catalogue { .. }));
        assert_eq!(with.rule.max_eirp_mbm, 2000);
        assert_eq!(with.rule.flags.contains(Flags::DFS), number >= 52);
    }
    // 2.4 GHz as well: the catalogue's 20 dBm stands under a 30 dBm owner's rule.
    let low = OwnerRule::new(*b"UZ", 2_400_000, 2_500_000, 40_000, 3000, Flags::AUTO_BW).unwrap();
    for number in 1..=13 {
        assert_eq!(uz.plan(&[low], center_khz(number), 20_000).unwrap().rule.max_eirp_mbm, 2000);
    }
    // Channel 14 (2474-2494 MHz) crosses the catalogue's edge at 2482 MHz: still outside the rules.
    assert_eq!(uz.plan(&[low], center_khz(14), 20_000), Err(Error::OutsideRules));
    // 5330-5350 MHz no catalogue rule touches: the owner's. A 40 MHz channel across 5330 MHz: the catalogue's edge stands.
    assert_eq!(uz.plan(&[wide], 5_340_000, 20_000).unwrap().source, Source::Owner { index: 0 });
    assert_eq!(uz.plan(&[wide], 5_330_000, 40_000), Err(Error::OutsideRules));
    // Below the catalogue's 5170 MHz edge: 5150-5170 MHz is the owner's, 5160-5180 crosses the edge.
    assert_eq!(uz.plan(&[wide], 5_160_000, 20_000).unwrap().source, Source::Owner { index: 0 });
    assert_eq!(uz.plan(&[wide], 5_170_000, 20_000), Err(Error::OutsideRules));
}

#[test]
fn an_owners_rule_is_checked_as_a_catalogue_line_and_the_catalogue_is_untouched() {
    let rule = |code: &[u8; 2], start, end, width, power| OwnerRule::new(*code, start, end, width, power, Flags::DFS);
    assert_eq!(rule(b"00", 5_470_000, 5_725_000, 80_000, 2000), Err(Error::WorldIsNotCountry));
    assert_eq!(rule(b"ZZ", 5_470_000, 5_725_000, 80_000, 2000), Err(Error::UnknownCountry));
    assert_eq!(rule(b"uz", 5_470_000, 5_725_000, 80_000, 2000), Err(Error::CountryFormat));
    for (start, end, width, power) in [(5_470_000, 5_470_000, 20_000, 2000), (5_725_000, 5_470_000, 20_000, 2000),
        (5_470_000, 5_480_000, 20_000, 2000), (5_470_000, 5_725_000, 0, 2000), (5_470_000, 5_725_000, 80_000, 0),
        (5_470_000, 5_725_000, 80_000, -100)] {
        assert_eq!(rule(b"UZ", start, end, width, power), Err(Error::OwnerRule), "{start}-{end} @{width} ({power})");
    }
    let made = bench();
    assert_eq!(made.country(), *b"UZ");
    assert_eq!((made.rule().start_khz, made.rule().end_khz, made.rule().max_bandwidth_khz), (5_470_000, 5_725_000, 80_000));
    assert!(Flags::DFS.with(Flags::NO_IR).contains(Flags::DFS) && Flags::DFS.with(Flags::NO_IR).contains(Flags::NO_IR));
    // The catalogue's file is the upstream snapshot it was: its hash, and UZ's three rules as before.
    let hex: String = sha256::digest(include_bytes!("../regdb/db.txt")).iter().map(|b| format!("{b:02x}")).collect();
    assert_eq!(hex, regulatory::SOURCE_SHA256);
    let uz: Vec<_> = country(b"UZ").source_rules().map(|(r, _)| (r.start_khz, r.end_khz, r.max_eirp_mbm)).collect();
    assert_eq!(uz, [(2_402_000, 2_482_000, 2000), (5_170_000, 5_250_000, 2000), (5_250_000, 5_330_000, 2000)]);
}
