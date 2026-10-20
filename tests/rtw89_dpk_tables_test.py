import importlib.util
from pathlib import Path
import unittest

ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location('dpk_tables', ROOT / 'scripts/hardware/rtw89_dpk_tables.py')
tables = importlib.util.module_from_spec(spec)
spec.loader.exec_module(tables)

class PinnedTables(unittest.TestCase):
    def test_foreign_or_modified_source_cannot_generate_register_writes(self):
        for raw in (b'', b'rtw8852b_dpk_afe_defs', b'\xff' * 32):
            with self.assertRaisesRegex(ValueError, 'SHA-256'):
                tables.extract(raw)

    def test_source_table_shape_and_masked_values_remain_valid(self):
        import re
        text = (ROOT / 'rtw89_wifi/src/dpk_tables.rs').read_text()
        blocks = re.findall(r'const (\w+): &\[\(u32,u32,u32\)\] = &\[(.*?)\];', text, re.S)
        self.assertEqual([name for name, _ in blocks], ['AFE', 'AFE_RESTORE', 'KIP'])
        rows = [[tuple(int(v, 16) for v in row) for row in re.findall(r'\((0x\w+),(0x\w+),(0x\w+)\)', body)] for _, body in blocks]
        self.assertEqual([len(row) for row in rows], [29, 24, 2])
        self.assertEqual(rows[0][0], (0x20fc, 0xffff0000, 0x303))
        self.assertEqual(rows[1][-1], (0x78e4, 0x18000000, 2))
        self.assertEqual(rows[2], [(0x8008, 0xffffffff, 0), (0x8088, 0xffffffff, 0x80000000)])
        for row in rows:
            for at, mask, value in row:
                self.assertEqual(at % 4, 0)
                self.assertLess(at, 0x10000)
                shift = (mask & -mask).bit_length() - 1
                self.assertEqual(value << shift & ~mask, 0)

if __name__ == '__main__':
    unittest.main()
