import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('iqk_tables', ROOT / 'scripts/hardware/rtw89_iqk_tables.py')
tables = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tables)

class PinnedTables(unittest.TestCase):
    def test_foreign_or_mutated_source_cannot_be_extracted(self):
        for value in (b'', b'rtw8852b_set_nondbcc_path01', b'\xff' * 32):
            with self.assertRaisesRegex(ValueError, 'SHA-256'):
                tables.extract(value)

    def test_checked_in_tables_preserve_source_shape_and_transition_boundaries(self):
        import re
        text = (ROOT / 'rtw89_wifi/src/iqk_tables.rs').read_text()
        blocks = re.findall(r'const (\w+): &\[\(usize,u32,u32\)\] = &\[(.*?)\];', text, re.S)
        self.assertEqual([name for name, _ in blocks], ['SET_NONDBCC_PATH01', 'RESTORE_NONDBCC_PATH01'])
        rows = [[tuple(int(v, 16) for v in row) for row in re.findall(r'\((0x\w+),(0x\w+),(0x\w+)\)', body)] for _, body in blocks]
        self.assertEqual([len(row) for row in rows], [32, 18])
        self.assertEqual(rows[0][0], (0x20fc, 0xffff0000, 0x303))
        self.assertEqual(rows[0][-1], (0x20fc, 0xffff0000, 0x3333))
        self.assertEqual(rows[1][-1], (0x2344, 0x80000000, 0))
        for row in rows:
            for at, mask, value in row:
                self.assertEqual(at % 4, 0)
                shift = (mask & -mask).bit_length() - 1
                self.assertEqual(value << shift & ~mask, 0)

if __name__ == '__main__':
    unittest.main()
