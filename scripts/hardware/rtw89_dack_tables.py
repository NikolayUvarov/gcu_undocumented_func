#!/usr/bin/env python3
"""Reproduce only the pinned RTL8852B DACK tables, without network or device access."""
import argparse
import hashlib
from pathlib import Path
import re

PIN = 'ebc6840d2e15c7a9f12fb7d7121ca19bb8acff93524d3902a7ce892e0714fc21'
TABLES = [('afe_init_defs',10), ('check_addc_defs_a',6), ('check_addc_defs_b',6),
          ('check_dadc_en_defs_b',7), ('check_dadc_dis_defs_b',4),
          ('dack_s0_1_defs',18), ('dack_s0_2_defs',3), ('dack_s0_3_defs',5),
          ('dack_s1_1_defs',18), ('dack_s1_2_defs',4), ('dack_s1_3_defs',5)]

def number(value):
    value = value.strip()
    if re.fullmatch(r'0[xX][0-9a-fA-F]+|[0-9]+', value):
        result = int(value, 0)
    elif match := re.fullmatch(r'BIT\(([0-9]+)\)', value):
        bit = int(match[1])
        if bit >= 32: raise ValueError('invalid BIT')
        result = 1 << bit
    elif value in ('RF_PATH_A', 'RF_PATH_B'):
        result = int(value == 'RF_PATH_B')
    else:
        raise ValueError('unsupported expression: '+value)
    if not 0 <= result <= 0xffffffff: raise ValueError('out of range')
    return result

def parse_body(body):
    rows=[]
    pattern=re.compile(r'\s*RTW89_DECL_RFK_(WM|WRF|DELAY)\(([^\n]+)\),\s*')
    while body.strip():
        match=pattern.match(body)
        if not match: raise ValueError('unrecognized table record')
        kind=match[1]; args=[number(value) for value in match[2].split(',')]
        if len(args) != {'WM':3,'WRF':4,'DELAY':1}[kind]: raise ValueError('wrong argument count')
        if kind=='DELAY':
            if args[0]!=1: raise ValueError('unreviewed delay')
        else:
            at,mask,value=args[-3:]
            if kind=='WM' and (at & 3 or at>=0x10000): raise ValueError('invalid BB address')
            if kind=='WRF' and (args[0]>1 or at&~0x100ff or mask>0xfffff): raise ValueError('invalid RF address or mask')
            if not mask: raise ValueError('zero field mask')
            shift=(mask&-mask).bit_length()-1
            if value > (mask>>shift) or (value<<shift)&~mask: raise ValueError('value outside field')
        rows.append((kind,args)); body=body[match.end():]
    return rows

def extract(raw):
    if hashlib.sha256(raw).hexdigest()!=PIN: raise ValueError('source SHA-256 mismatch')
    source=raw.decode('ascii'); result=[]
    for name,count in TABLES:
        matches=re.findall(r'static const struct rtw89_reg5_def rtw8852b_'+name+r'\[\] = \{(.*?)\n\};',source,re.S)
        if len(matches)!=1: raise ValueError('missing or duplicate table '+name)
        rows=parse_body(matches[0])
        if len(rows)!=count: raise ValueError('unexpected row count '+name)
        result.append((name,rows))
    return result

def render(tables):
    lines=['// SPDX-License-Identifier: BSD-3-Clause', '// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.',
           '// Generated from pinned RTL8852B RFK tables by scripts/hardware/rtw89_dack_tables.py.',
           '#[derive(Clone, Copy, Debug, Eq, PartialEq)]',
           'pub(crate) enum Op { Bb(u32,u32,u32), Rf(u8,u32,u32,u32), Delay(u32) }', '']
    for name,rows in tables:
        lines.append('pub(crate) const '+name.upper()+': &[Op] = &[')
        for kind,args in rows:
            label={'WM':'Bb','WRF':'Rf','DELAY':'Delay'}[kind]
            lines.append('    Op::'+label+'('+', '.join(f'0x{x:08x}' for x in args)+'),')
        lines.extend(['];',''])
    return '\n'.join(lines)

def main():
    p=argparse.ArgumentParser(); p.add_argument('source',type=Path); p.add_argument('--check',action='store_true'); args=p.parse_args()
    target=Path(__file__).resolve().parents[2]/'rtw89_wifi/src/dack_tables.rs'
    result=render(extract(args.source.read_bytes()))
    if args.check:
        if target.read_text()!=result: raise SystemExit('DACK table regeneration mismatch')
        print('pinned RTL8852B DACK tables match')
    else:
        target.write_text(result)

if __name__=='__main__': main()
