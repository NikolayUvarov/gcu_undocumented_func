#!/usr/bin/env python3
"""Reproduce the two pinned initial RTL8852B IQK register tables offline."""
import argparse
import hashlib
from pathlib import Path
import re

PIN = 'c4613e3d2755f14649154a1c8393bf72ae215e8b683219a0763a033b224c715f'
TABLES = [('set_nondbcc_path01', 32), ('restore_nondbcc_path01', 18)]

def extract(raw):
    if hashlib.sha256(raw).hexdigest() != PIN:
        raise ValueError('source SHA-256 mismatch')
    source = raw.decode('ascii')
    result = []
    for name, count in TABLES:
        matches = re.findall(r'static const struct rtw89_reg3_def rtw8852b_' + name + r'\[\] = \{(.*?)\n\};', source, re.S)
        if len(matches) != 1:
            raise ValueError('missing or duplicate table')
        body = matches[0]
        rows = []
        while body.strip():
            m = re.match(r'\s*\{(0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+)\},\s*', body)
            if not m:
                raise ValueError('unrecognized table entry')
            at, mask, value = (int(v, 16) for v in m.groups())
            if at & 3 or at >= 0x10000 or not 0 < mask <= 0xffffffff:
                raise ValueError('invalid address or mask')
            shift = (mask & -mask).bit_length() - 1
            if value << shift & ~mask:
                raise ValueError('field overflow')
            rows.append((at, mask, value))
            body = body[m.end():]
        if len(rows) != count:
            raise ValueError('table length mismatch')
        result.append((name, rows))
    return result

def render(tables):
    lines = ['// SPDX-License-Identifier: BSD-3-Clause',
             '// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.',
             '// Generated from pinned rtw8852b_rfk.c by scripts/hardware/rtw89_iqk_tables.py.', '']
    for name, rows in tables:
        lines.append('pub(crate) const ' + name.upper() + ': &[(usize,u32,u32)] = &[')
        lines.extend('    (0x%04x,0x%08x,0x%x),' % row for row in rows)
        lines.extend(['];', ''])
    return '\n'.join(lines)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    target = Path(__file__).resolve().parents[2] / 'rtw89_wifi/src/iqk_tables.rs'
    result = render(extract(args.source.read_bytes()))
    if args.check:
        if target.read_text() != result:
            raise SystemExit('IQK table regeneration mismatch')
        print('pinned RTL8852B IQK tables match')
    else:
        target.write_text(result)

if __name__ == '__main__':
    main()
