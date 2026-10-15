#!/usr/bin/env python3
import importlib.util
from pathlib import Path
import sys
import unittest
sys.dont_write_bytecode = True
spec=importlib.util.spec_from_file_location('extract',Path(__file__).resolve().parents[1]/'scripts/hardware/rtw89_dack_tables.py')
extract=importlib.util.module_from_spec(spec)
spec.loader.exec_module(extract)

class Tables(unittest.TestCase):
    def test_fields_delays_paths_and_order(self):
        rows=extract.parse_body('RTW89_DECL_RFK_WM(0xc004, BIT(31), 1),\nRTW89_DECL_RFK_WRF(RF_PATH_B, 0x8f, BIT(13), 0),\nRTW89_DECL_RFK_DELAY(1),')
        self.assertEqual(rows,[('WM',[0xc004,1<<31,1]),('WRF',[1,0x8f,1<<13,0]),('DELAY',[1])])
    def test_unknown_effects_cannot_be_silently_omitted(self):
        for text in ['RTW89_DECL_RFK_WC(0xc004,1),','RTW89_DECL_RFK_DELAY(2),','RTW89_DECL_RFK_WM(0xc003, 1, 0),','RTW89_DECL_RFK_WM(0xc004, 0, 0),','RTW89_DECL_RFK_WM(0xc004, BIT(32), 0),','RTW89_DECL_RFK_WRF(RF_PATH_B, 0x20000, 1, 0),','RTW89_DECL_RFK_WM(0xc004, 5, 2),']:
            with self.subTest(text=text),self.assertRaises(ValueError):extract.parse_body(text)
    def test_full_source_hash_is_checked_before_parsing(self):
        with self.assertRaisesRegex(ValueError,'SHA-256'):extract.extract(b'plausible but different tables')

if __name__=='__main__':unittest.main()
