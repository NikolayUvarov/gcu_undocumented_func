#![allow(dead_code)]
#[path = "../rtw89_wifi/src/regulatory.rs"]
mod regulatory;

use regulatory::{Assessment, DfsRegion, Environment, Error, Flags, Restriction};

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
