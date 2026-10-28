#!/usr/bin/env python3
"""Offline fail-closed checks for the pinned table extractor; no firmware or downloads."""
import importlib.util
from pathlib import Path
import unittest
import sys

sys.dont_write_bytecode = True

spec = importlib.util.spec_from_file_location("rtw89_phy_tables", Path(__file__).resolve().parents[1] / "scripts/hardware/rtw89_phy_tables.py")
tables = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tables)


class TableExtraction(unittest.TestCase):
    def test_selects_exact_cut_one_instead_of_cut_zero_and_keeps_duplicate_writes(self):
        rows = [(0xf0010000, 0), (0xf0010001, 1), (0x10, 0x11),
                (0x80010000, 0), (0x40000000, 0), (0x20, 0xaa),
                (0x90010001, 0), (0x40000000, 0), (0x20, 0xbb), (0x20, 0xcc),
                (0xa0000000, 0), (0xb0000000, 0), (0x30, 0xdd)]
        self.assertEqual(tables.select(rows), [(0x10, 0x11), (0x20, 0xbb), (0x20, 0xcc), (0x30, 0xdd)])

    def test_unknown_board_cannot_fall_back_to_a_different_cut(self):
        with self.assertRaises(ValueError):
            tables.select([(0xf0010000, 0), (0xf0ff00ff, 0), (0x10, 0)])

    def test_malformed_and_unmatched_control_flow_is_rejected(self):
        for rows in [[(0x80010001, 0)], [(0x40000000, 0)], [(0xb0000000, 0)],
                     [(0x80010001, 0), (0x10, 0)], [(0x80010001, 0), (0x80010001, 0)],
                     [(0x80010000, 0), (0x40000000, 0), (0xa0000000, 0)], [(0xc0000000, 0)]]:
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                tables.select([(0xf0010001, 0)] + rows)

    def test_source_hash_checked_before_any_c_syntax_is_accepted(self):
        with self.assertRaisesRegex(ValueError, "SHA-256"):
            tables.extract(b"static const struct rtw89_reg2_def rtw89_8852b_phy_bb_regs[] = {};")


if __name__ == "__main__":
    unittest.main()
