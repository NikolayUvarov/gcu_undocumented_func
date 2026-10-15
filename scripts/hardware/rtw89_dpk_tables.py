#!/usr/bin/env python3
"""Reproduce the three pinned RTL8852B initial DPK tables offline."""
import argparse
import hashlib
from pathlib import Path
import re

PIN = 'ebc6840d2e15c7a9f12fb7d7121ca19bb8acff93524d3902a7ce892e0714fc21'
TABLES = [('afe', 29), ('afe_restore', 24), ('kip', 2)]

def extract(raw):
    if hashlib.sha256(raw).hexdigest() != PIN:
        raise ValueError('source SHA-256 mismatch')
    source = raw.decode('ascii')
    result = []
    for name, count in TABLES:
        matches = re.findall(r'static const struct rtw89_reg5_def rtw8852b_dpk_' + name + r'_defs\[\] = \{(.*?)\n\};', source, re.S)
        if len(matches) != 1:
            raise ValueError('missing or duplicate table')
        body, rows = matches[0], []
        while body.strip():
            match = re.match(r'\s*RTW89_DECL_RFK_WM\((0x[0-9a-fA-F]+),\s*(BIT\(\d+\)|0x[0-9a-fA-F]+),\s*(0x[0-9a-fA-F]+)\),\s*', body)
            if not match:
                raise ValueError('unrecognized table entry')
            address, mask, value = match.groups()
            at, value = int(address, 16), int(value, 16)
            mask = 1 << int(mask[4:-1]) if mask.startswith('BIT') else int(mask, 16)
            if at & 3 or at >= 0x10000 or not 0 < mask <= 0xffffffff:
                raise ValueError('invalid address or mask')
            shift = (mask & -mask).bit_length() - 1
            if value << shift & ~mask:
                raise ValueError('field overflow')
            rows.append((at, mask, value))
            body = body[match.end():]
        if len(rows) != count:
            raise ValueError('table length mismatch')
        result.append((name, rows))
    return result

def render(tables):
    lines = ['// SPDX-License-Identifier: BSD-3-Clause',
             '// Copyright(c) 2019-2022 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.',
             '// Generated from pinned rtw8852b_rfk_table.c by scripts/hardware/rtw89_dpk_tables.py.', '']
    for name, rows in tables:
        lines.append('pub(crate) const ' + name.upper() + ': &[(u32,u32,u32)] = &[')
        lines.extend('    (0x%04x,0x%08x,0x%x),' % row for row in rows)
        lines.extend(['];', ''])
    return '\n'.join(lines)

def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('source', type=Path)
    parser.add_argument('--check', action='store_true')
    args = parser.parse_args()
    target = Path(__file__).resolve().parents[2] / 'rtw89_wifi/src/dpk_tables.rs'
    result = render(extract(args.source.read_bytes()))
    if args.check:
        if target.read_text() != result:
            raise SystemExit('DPK table regeneration mismatch')
        print('pinned RTL8852B DPK tables match')
    else:
        target.write_text(result)

if __name__ == '__main__':
    main()
