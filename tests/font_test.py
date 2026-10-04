#!/usr/bin/env python3
"""MIND Mono 16: coverage of the subset, licence notice and freshness of common/font16.rs."""
import re
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import font_gen  # noqa: E402


class Font(unittest.TestCase):
    def setUp(self):
        self.header, self.glyphs = font_gen.parse_bdf(font_gen.SUBSET.read_text(encoding="utf-8"))

    def test_coverage(self):
        need = "".join(chr(c) for c in range(0x20, 0x7F)) + "АБВГДЕЁЖЗИЙКЛМНОПРСТУФХЦЧШЩЪЫЬЭЮЯабвгдеёжзийклмнопрстуфхцчшщъыьэюя"
        need += "─│┌┐└┘├┤┬┴┼═║╔╗╚╝╠╣╦╩╬█▀▄▌▐░▒▓←↑→↓…—«»№✓⣿�"
        missing = [ch for ch in need if ord(ch) not in self.glyphs]
        self.assertEqual(missing, [])

    def test_cells_are_8x16_and_not_empty(self):
        for code, (_, rows) in self.glyphs.items():
            self.assertEqual(len(rows), 16)
            if code not in (0x20, 0xA0, 0xAD, 0x2800) and not 0x2000 <= code <= 0x200F:
                self.assertTrue(any(rows), hex(code))

    def test_reserved_name_not_used_and_copyright_kept(self):
        header = "\n".join(self.header)
        self.assertNotIn("Terminus\"", header.replace("Terminus Font 4.49.1", ""))
        self.assertIn('FAMILY_NAME "MIND Mono"', header)
        self.assertIn("Copyright (C) 2020 Dimitar Toshkov Zhekov", header)
        self.assertTrue((ROOT / "fonts" / "OFL.txt").read_text().startswith("Copyright (C) 2020 Dimitar Toshkov Zhekov"))

    def test_generated_table_is_fresh_and_sorted(self):
        result = subprocess.run([sys.executable, str(ROOT / "scripts" / "font_gen.py"), "--check"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)
        codes = [int(c, 16) for c in re.findall(r"0x([0-9A-F]{4})", (ROOT / "common" / "font16.rs").read_text().split("pub static GLYPHS")[0])]
        self.assertEqual(codes, sorted(set(codes)))


if __name__ == "__main__":
    unittest.main()
