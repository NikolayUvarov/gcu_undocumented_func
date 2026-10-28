#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import sys
import unittest
sys.dont_write_bytecode = True
sys.path.insert(0, str(Path(__file__).resolve().parents[1]/'scripts/hardware'))
import rtw89_tssi_tables as extract

class Tables(unittest.TestCase):
    def test_complete_pmac_records_retain_order_and_masks(self):
        self.assertEqual(extract.triples('{0x980, BIT(0), 1}, {0x980, BIT(16), 0},'),
                         [[0x980,1,1],[0x980,1<<16,0]])
    def test_extra_or_unknown_effects_are_not_omitted(self):
        for raw in ['{0x980,1,0}, UNREVIEWED(1),', '{0x981,1,0},', '{0x980,0,0},',
                    '{0x980,5,2},', '{0x980,1,0,3},', '{0x980,1,0}']:
            with self.subTest(raw=raw), self.assertRaises(ValueError): extract.triples(raw)
    def test_signed_thermal_length_range_and_full_parse(self):
        self.assertEqual(extract.signed_bytes(','.join(['-2']*30)),[-2]*30)
        for raw in [','.join(['0']*29), ','.join(['128']*30), ','.join(['0']*30)+', tail', '0,'*29+'-129']:
            with self.subTest(raw=raw), self.assertRaises(ValueError): extract.signed_bytes(raw)
    def test_duplicate_initializers_are_rejected(self):
        with self.assertRaises(ValueError): extract.one('a{x} a{y}',r'a\{(.*?)\}')
    def test_each_source_is_pinned_before_data_is_used(self):
        with self.assertRaisesRegex(ValueError,'SHA-256'):
            extract.extract({name:b'untrusted input' for name in extract.PINS})

if __name__ == '__main__': unittest.main()
