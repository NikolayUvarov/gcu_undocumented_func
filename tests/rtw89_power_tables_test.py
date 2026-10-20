#!/usr/bin/env python3
"""Offline checks of strict C parsing and preserved power-table sentinels."""
import importlib.util
from pathlib import Path
import sys
import unittest
sys.dont_write_bytecode = True
spec = importlib.util.spec_from_file_location("power_tables", Path(__file__).resolve().parents[1] / "scripts/hardware/rtw89_power_tables.py")
tables = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tables)


class PowerTables(unittest.TestCase):
    def test_signed_values_explicit_zero_and_source_index_order(self):
        got = tables.designated("[1][RTW89_FCC][13] = -7,\n[0][RTW89_WW][0] = 0,", (2, 16, 14))
        self.assertEqual(got, {(1, 2, 13): -7, (0, 0, 0): 0})
        self.assertEqual(tables.flat((1, 2, 13), (2, 16, 14)), 265)

    def test_designated_parser_rejects_unconsumed_duplicate_and_out_of_range_data(self):
        for source in ["[0] = 0, [0] = 1,", "[2] = 0,", "[0][0] = 0,", "[0] = -129,", "[0] = 128,",
                       "[0] = 0;", "[0] = fn(),", "[RTW89_UNKNOWN] = 0,", "[0] = 0, garbage", ""]:
            with self.subTest(source=source), self.assertRaises(ValueError):
                tables.designated(source, (2,))

    def test_by_rate_decodes_little_endian_signed_bytes_and_disjoint_offsets(self):
        got = tables.byrate("{ 0, 0, 2, 4, 4, 0xff807f01, }, { 0, 0, 4, 0, 1, 0x00, },")
        self.assertEqual(got, {(0, 0, 2, 4): 1, (0, 0, 2, 5): 127, (0, 0, 2, 6): -128, (0, 0, 2, 7): -1, (0, 0, 4, 0): 0})

    def test_byrate_invalid_ranges_payload_and_duplicates_refuse(self):
        row = "{ 0, 0, 0, 0, 4, 0x50505050, },"
        for source in [row + row, "{ 0, 1, 0, 0, 1, 0x00, },", "{ 0, 0, 2, 11, 2, 0x00, },",
                       "{ 0, 0, 0, 0, 1, 0x100, },", "{ 0, 0, 0, 0, 0, 0x00, },", row + "bad"]:
            with self.subTest(source=source), self.assertRaises(ValueError):
                tables.byrate(source)

    def test_country_groups_do_not_discard_band_differences_or_flags(self):
        row = 'COUNTRY_REGD("US", RTW89_FCC, RTW89_FCC, RTW89_FCC, 0x1),'
        self.assertEqual(tables.countries(row + '\nCOUNTRY_REGD("RU", RTW89_ETSI, RTW89_ETSI, RTW89_NA, 0x0),'),
                         {"US": (2, 2, 2, 1), "RU": (1, 1, 4, 0)})
        for source in [row + row, row.replace("0x1", "0x4"), row.replace("FCC", "TYPO"), row + "bad"]:
            with self.subTest(source=source), self.assertRaises(ValueError):
                tables.countries(source)

    def test_exact_unique_table_is_required(self):
        source = "const s8 x[A][B] = {\n[0][0] = 1,\n};"
        self.assertEqual(tables.body(source, "x").strip(), "[0][0] = 1,")
        for bad in [source + source, source.replace("x[A]", "y[A]")]:
            with self.assertRaises(ValueError):
                tables.body(bad, "x")

    def test_both_sources_must_match_pins_before_parsing(self):
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            tables.extract(b"not pinned", b"not pinned")


if __name__ == "__main__":
    unittest.main()
