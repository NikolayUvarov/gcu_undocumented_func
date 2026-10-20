#!/usr/bin/env python3
"""Regress strict source ingestion and the committed country/rule projection."""
import importlib.util
import hashlib
from pathlib import Path
import re
import sys
import unittest

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("regdb", ROOT / "scripts/rtw89_regdb.py")
regdb = importlib.util.module_from_spec(SPEC)
sys.modules[SPEC.name] = regdb
SPEC.loader.exec_module(regdb)

BASE = "country UZ: DFS-ETSI\n (2402 - 2482 @ 40), (20)\n"


class CatalogTests(unittest.TestCase):
    def test_exact_isc_notice_is_retained(self):
        license_bytes = (ROOT / "LICENSES/ISC-wireless-regdb.txt").read_bytes()
        self.assertEqual(hashlib.sha256(license_bytes).hexdigest(),
                         "678b0df753c86198fc496d1f1033429bbd57f101472132ee7eaaf9f5e0a7fae1")

    def test_pinned_full_source_and_deterministic_output(self):
        source = regdb.SOURCE.read_bytes()
        generated = regdb.generate(source)
        self.assertEqual(generated, regdb.OUTPUT.read_text())
        self.assertEqual(generated, regdb.generate(source))
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            regdb.generate(source + b"\n")

    def test_all_countries_and_all_rules_are_retained(self):
        text = regdb.SOURCE.read_text()
        countries, wmms = regdb.parse(text)
        headers = re.findall(r"^country ([A-Z0-9]{2}):", text, re.M)
        self.assertEqual(set(countries), set(headers))
        self.assertEqual(len(countries), 182)
        self.assertEqual(sum(len(country.rules) for country in countries.values()), 1016)
        self.assertEqual(len(wmms["ETSI"]), 8)
        for code in ("IR", "OM"):
            self.assertTrue(any(rule.flags & (1 << regdb.FLAGS.index("NO-INDOOR"))
                                for rule, _ in countries[code].rules))

    def test_exact_khz_and_conservative_decimal_milliwatt_units(self):
        self.assertEqual(regdb.khz("2483.5"), 2483500)
        self.assertEqual(regdb.power("500", True), 2698)
        self.assertEqual(regdb.power("2", True), 301)
        self.assertEqual(regdb.power("23.00"), 2300)
        self.assertEqual(regdb.power("-0.001"), -1)
        for literal in ("100mW", "100 mW"):
            countries, _ = regdb.parse(BASE.replace("(20)", f"({literal})"))
            self.assertEqual(countries["UZ"].rules[0][0].eirp, 2000)
        for value in ("0", "-1", "NaN", "Infinity"):
            with self.assertRaises(ValueError):
                regdb.power(value, True)
        with self.assertRaises(ValueError):
            regdb.khz("2402.0001")

    def test_unknown_or_malformed_flag_cannot_be_ignored(self):
        for extra in ("NO-FUTURE", "NO-IR,", "NO-IR, NO-IR", "wmmrule=MISSING"):
            with self.assertRaises(ValueError):
                regdb.parse(BASE.rstrip() + ", " + extra)

    def test_duplicate_country_or_rule_is_not_merged(self):
        for text in (BASE + BASE, BASE + " (2402 - 2482 @ 40), (20)\n"):
            with self.assertRaisesRegex(ValueError, "duplicate"):
                regdb.parse(text)

    def test_missing_country_invalid_units_and_unreviewed_syntax_fail(self):
        for text in (" (2402 - 2482 @ 40), (20)", "country UZ:",
                     BASE.replace("(20)", "(20 W)"), BASE + "quietly skip me\n",
                     BASE.replace("DFS-ETSI", "DFS-NEW"), BASE.replace("UZ", "uz")):
            with self.assertRaises(ValueError):
                regdb.parse(text)

    def test_invalid_bandwidth_or_range_cannot_authorize_center_only(self):
        for band in ("2482 - 2402 @ 40", "2402 - 2412 @ 20", "0 - 2482 @ 20"):
            with self.assertRaises(ValueError):
                regdb.parse(BASE.replace("2402 - 2482 @ 40", band))

    def test_full_width_overlap_requires_review_instead_of_most_permissive_match(self):
        with self.assertRaisesRegex(ValueError, "ambiguous"):
            regdb.parse(BASE + " (2400 - 2483.5 @ 40), (30)\n")
        countries, _ = regdb.parse(BASE + " (2474 - 2494 @ 20), (20), NO-OFDM\n")
        self.assertEqual(len(countries["UZ"].rules), 2)

    def test_wmm_incomplete_unknown_or_duplicate_values_are_rejected(self):
        prefix = "wmmrule ETSI:\n vo_c: cw_min=3, cw_max=7, aifsn=2, cot=2\n"
        for text in (prefix + BASE, prefix + prefix + BASE):
            with self.assertRaises(ValueError):
                regdb.parse(text)
        complete = regdb.SOURCE.read_text().split("country 00:", 1)[0] + BASE
        self.assertEqual(len(regdb.parse(complete)[1]["ETSI"]), 8)
        for value in ("0", "2"):
            with self.assertRaisesRegex(ValueError, "contention window"):
                regdb.parse(complete.replace("cw_min=3", "cw_min=" + value, 1))
        with self.assertRaisesRegex(ValueError, "unknown WMM fields"):
            regdb.parse(complete.replace("cot=2", "unknown=2", 1))

    def test_no_indoor_is_preserved_even_when_upstream_binary_parser_drops_it(self):
        countries, _ = regdb.parse(BASE + " (5150 - 5250 @ 80), (23), NO-INDOOR\n")
        self.assertEqual(len(countries["UZ"].rules), 2)
        self.assertTrue(countries["UZ"].rules[1][0].flags & (1 << regdb.FLAGS.index("NO-INDOOR")))


if __name__ == "__main__":
    unittest.main()
