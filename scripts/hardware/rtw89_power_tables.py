#!/usr/bin/env python3
"""Extract pinned RTL8852B default 2 GHz power data; this is not a channel policy."""
import argparse
import hashlib
from pathlib import Path
import re

COMMIT = "05f7e89ab9731565d8a62e3b5d1ec206485eeb0b"
POWER_SHA256 = "8b9b5fc32628037d8dec86cff976913164723d87abe38fd09d229c57b509b42f"
REGD_SHA256 = "1d7397fecb4716c6053db7eb9d032483445f41da7200166d4a7e4be4cb1411b7"
REGIONS = ("WW", "ETSI", "FCC", "MKK", "NA", "IC", "KCC", "ACMA", "NCC",
           "MEXICO", "CHILE", "UKRAINE", "CN", "QATAR", "UK", "THAILAND")
INDEX = re.compile(r"\[\s*(RTW89_[A-Z0-9_]+|[0-9]+)\s*\]")
ENTRY = re.compile(r"((?:\[\s*(?:RTW89_[A-Z0-9_]+|[0-9]+)\s*\])+)[ \t]*=[ \t]*(-?[0-9]+)\s*,")
COUNTRY = re.compile(r'COUNTRY_REGD\("([A-Z]{2})",\s*(RTW89_[A-Z]+),\s*(RTW89_[A-Z]+),\s*(RTW89_[A-Z]+),\s*(0x[0-9a-f]+)\),')
BYRATE = re.compile(r"\{\s*([0-9]+),\s*([0-9]+),\s*([0-9]+),\s*([0-9]+),\s*([0-9]+),\s*(0x[0-9a-f]+),\s*\},")


def region(token):
    if not token.startswith("RTW89_") or token[6:] not in REGIONS:
        raise ValueError("unknown regulatory power group")
    return REGIONS.index(token[6:])


def body(source, name):
    matches = re.findall(r"\b" + re.escape(name) + r"(?:\s*\[[A-Z0-9_]*\])+\s*=\s*\{(.*?)\n\};", source, re.S)
    if len(matches) != 1:
        raise ValueError(f"missing or duplicate {name}")
    return matches[0]


def designated(source, dimensions):
    if ENTRY.sub("", source).strip():
        raise ValueError("unrecognized designated initializer")
    result = {}
    for indices, raw in ENTRY.findall(source):
        coords = tuple(region(s) if s.startswith("RTW89_") else int(s)
                       for s in INDEX.findall(indices))
        if len(coords) != len(dimensions) or any(x >= n for x, n in zip(coords, dimensions)):
            raise ValueError("power table index out of bounds")
        value = int(raw)
        if not -128 <= value <= 127 or coords in result:
            raise ValueError("duplicate index or invalid signed power byte")
        result[coords] = value
    if not result:
        raise ValueError("empty power table")
    return result


def countries(source):
    if COUNTRY.sub("", source).strip():
        raise ValueError("unrecognized country map")
    result = {}
    for name, a, b, c, flags in COUNTRY.findall(source):
        if name in result or int(flags, 16) & ~3:
            raise ValueError("duplicate country or unknown function flags")
        result[name] = (region(a), region(b), region(c), int(flags, 16))
    if not result:
        raise ValueError("empty country map")
    return result


def byrate(source):
    if BYRATE.sub("", source).strip():
        raise ValueError("unrecognized by-rate initializer")
    result = {}
    for row in BYRATE.findall(source):
        band, nss, rs, shift, length = map(int, row[:5])
        data = int(row[5], 16)
        if band > 1 or nss > 1 or rs > 4 or not 1 <= length <= 4 or data > 0xffffffff:
            raise ValueError("by-rate header out of bounds")
        if (rs in (0, 1, 4) and nss != 0) or shift + length > (4, 8, 12, 4, 5)[rs]:
            raise ValueError("invalid by-rate range")
        if data >> (length * 8):
            raise ValueError("unused by-rate payload")
        for i in range(length):
            key = (band, nss, rs, shift + i)
            if key in result:
                raise ValueError("duplicate by-rate slot")
            raw = (data >> (i * 8)) & 255
            result[key] = raw if raw < 128 else raw - 256
    if not result:
        raise ValueError("empty by-rate table")
    return result


def extract(power, regd):
    if hashlib.sha256(power).hexdigest() != POWER_SHA256 or hashlib.sha256(regd).hexdigest() != REGD_SHA256:
        raise ValueError("source SHA-256 differs from pinned power/country source")
    p, r = power.decode("ascii"), regd.decode("ascii")
    tables = {
        "LIMIT_2G": (designated(body(p, "rtw89_8852b_txpwr_lmt_2g"), (2, 2, 3, 2, 16, 14)), (2, 2, 3, 2, 16, 14)),
        "RU_2G": (designated(body(p, "rtw89_8852b_txpwr_lmt_ru_2g"), (5, 2, 16, 14)), (5, 2, 16, 14)),
    }
    shape = designated(body(p, "rtw89_8852b_tx_shape_lmt"), (3, 2, 16))
    shape_ru = designated(body(p, "rtw89_8852b_tx_shape_lmt_ru"), (3, 16))
    if any(v < 0 or v > 3 for v in (*shape.values(), *shape_ru.values())):
        raise ValueError("unknown transmit shape")
    tables["SHAPE_2G"] = ({k[1:]: v for k, v in shape.items() if k[0] == 0}, (2, 16))
    tables["SHAPE_RU_2G"] = ({k[1:]: v for k, v in shape_ru.items() if k[0] == 0}, (16,))
    rates = byrate(body(p, "rtw89_8852b_txpwr_byrate"))
    tables["BYRATE_2G"] = ({k[1:]: v for k, v in rates.items() if k[0] == 0}, (2, 5, 12))
    mapping = countries(body(r, "rtw89_regd_map"))
    if any(k[0] >= 3 for k in tables["RU_2G"][0]):
        raise ValueError("new RU format requires review")
    return tables, mapping


def flat(coords, dimensions):
    value = 0
    for index, dimension in zip(coords, dimensions):
        value = value * dimension + index
    return value


def render(tables, mapping):
    lines = ["// SPDX-License-Identifier: BSD-3-Clause",
             "// Copyright(c) 2019-2020 Realtek Corporation; see LICENSES/BSD-3-Clause-rtw89.txt.",
             "// Generated by scripts/hardware/rtw89_power_tables.py; do not edit.",
             f"// Linux {COMMIT}; power SHA-256 {POWER_SHA256}; regd SHA-256 {REGD_SHA256}.",
             "//! Default chip data only; no frequency permission or firmware override is implied.", ""]
    for name, (rows, dimensions) in tables.items():
        lines += [f"// Dimensions {dimensions}; omitted C entries are implicit zero.", f"pub const {name}: &[(u16, i8)] = &["]
        lines += [f"    ({flat(k, dimensions)}, {v})," for k, v in sorted(rows.items())]
        lines += ["];", ""]
    lines += ["pub const COUNTRIES: &[([u8; 2], [u8; 3], u8)] = &["]
    lines += [f'    (*b"{name}", [{a}, {b}, {c}], {flags}),' for name, (a, b, c, flags) in sorted(mapping.items())]
    return "\n".join(lines + ["];", ""])


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("power", type=Path)
    parser.add_argument("regd", type=Path)
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[2] / "rtw89_wifi/src/power_tables.rs")
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    result = render(*extract(args.power.read_bytes(), args.regd.read_bytes()))
    if args.check:
        if args.output.read_text() != result:
            raise SystemExit("generated default power/country data differs")
        print("pinned default power/country data matches")
    else:
        args.output.write_text(result)


if __name__ == "__main__":
    main()
