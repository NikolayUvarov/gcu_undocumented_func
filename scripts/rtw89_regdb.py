#!/usr/bin/env python3
"""Generate the reviewed wireless-regdb catalog without network or floating point."""
import argparse
from dataclasses import dataclass, field
from decimal import Decimal, InvalidOperation, ROUND_FLOOR, localcontext
import hashlib
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
COMMIT = "389b9b702cf9018e9a9078ccc4fa8eaae2e054e3"
SHA256 = "b71c815bd90c284131a27ab5af6fb4951b3580dc8af83345f31fcd34c5cd0581"
SOURCE = ROOT / "rtw89_wifi/regdb/db.txt"
OUTPUT = ROOT / "rtw89_wifi/src/regulatory_tables.rs"
FLAGS = ("NO-OFDM", "NO-CCK", "NO-INDOOR", "NO-OUTDOOR", "DFS", "PTP-ONLY",
         "PTMP-ONLY", "NO-IR", "NO-HT40", "AUTO-BW")
DFS = {None: "Unspecified", "DFS-FCC": "Fcc", "DFS-ETSI": "Etsi", "DFS-JP": "Jp"}
ACS = ("vo_c", "vi_c", "be_c", "bk_c", "vo_ap", "vi_ap", "be_ap", "bk_ap")
NUMBER = r"[0-9]+(?:\.[0-9]+)?"
RULE = re.compile(rf"\(({NUMBER})\s*-\s*({NUMBER})\s*@\s*({NUMBER})\),\s*"
                  rf"\((-?{NUMBER})(\s*mW)?\)(?:,\s*(.+))?")


@dataclass(frozen=True, order=True)
class Rule:
    start: int
    end: int
    bandwidth: int
    eirp: int
    flags: int
    wmm: str


@dataclass
class Country:
    dfs: str
    line: int
    rules: list = field(default_factory=list)


def khz(value):
    result = Decimal(value) * 1000
    if result != result.to_integral_value() or not 0 < result <= 100_000_000:
        raise ValueError("frequency is not an exact bounded kHz value")
    return int(result)


def power(value, milliwatts=False):
    try:
        number = Decimal(value)
        if not number.is_finite():
            raise ValueError("nonfinite power")
        with localcontext() as context:
            context.prec = 80
            if milliwatts:
                if not 0 < number <= 1_000_000:
                    raise ValueError("invalid milliwatt power")
                mbm = number.log10() * 1000
            else:
                mbm = number * 100
            result = int(mbm.to_integral_value(rounding=ROUND_FLOOR))
        if not -10_000 <= result <= 10_000:
            raise ValueError("power outside reviewed range")
        return result
    except InvalidOperation as error:
        raise ValueError("invalid decimal power") from error


def parse(source):
    countries, wmms = {}, {}
    current, wmm = [], None
    for number, raw in enumerate(source.splitlines(), 1):
        line = raw.partition("#")[0].strip()
        if not line:
            continue
        try:
            match = re.fullmatch(r"wmmrule ([A-Z][A-Z0-9_]*):", line)
            if match:
                if match[1] in wmms:
                    raise ValueError("duplicate WMM group")
                wmm, current = match[1], []
                wmms[wmm] = {}
                continue
            match = re.fullmatch(r"(vo|vi|be|bk)_(c|ap):\s*(.*)", line)
            if match:
                if wmm is None:
                    raise ValueError("WMM entry outside group")
                key = match[1] + "_" + match[2]
                if key in wmms[wmm]:
                    raise ValueError("duplicate WMM access category")
                pairs = [re.fullmatch(r"(cw_min|cw_max|aifsn|cot)=([0-9]+)", part.strip())
                         for part in match[3].split(",")]
                if len(pairs) != 4 or any(pair is None for pair in pairs):
                    raise ValueError("unknown WMM fields")
                values = {pair[1]: int(pair[2]) for pair in pairs}
                if len(values) != 4:
                    raise ValueError("duplicate WMM field")
                minimum, maximum, aifsn, cot = (values[key] for key in ("cw_min", "cw_max", "aifsn", "cot"))
                if any(not 1 <= value < 32768 or (value + 1) & value for value in (minimum, maximum)):
                    raise ValueError("invalid WMM contention window")
                if minimum > maximum or not 1 <= aifsn <= 255 or cot > 65535:
                    raise ValueError("invalid WMM timing")
                wmms[wmm][key] = (minimum, maximum, aifsn, cot)
                continue
            match = re.fullmatch(r"country ((?:[A-Z]{2}|00)(?:,(?:[A-Z]{2}|00))*):\s*(DFS-[A-Z]+)?", line)
            if match:
                if match[2] not in DFS:
                    raise ValueError("unknown DFS region")
                current, wmm = match[1].split(","), None
                for code in current:
                    if code in countries:
                        raise ValueError("duplicate country")
                    countries[code] = Country(DFS[match[2]], number)
                continue
            match = RULE.fullmatch(line)
            if match is None or not current or wmm is not None:
                raise ValueError("unrecognized regulatory syntax")
            start, end, bandwidth = (khz(match[index]) for index in (1, 2, 3))
            if start >= end or bandwidth > end - start:
                raise ValueError("invalid band or maximum bandwidth")
            flags, reference = 0, ""
            extras = match[6].split(",") if match[6] else []
            for index, extra in enumerate(extras):
                extra = extra.strip()
                if extra.startswith("wmmrule="):
                    reference = extra[len("wmmrule="):]
                    if index != len(extras) - 1 or reference not in wmms:
                        raise ValueError("unknown or misplaced WMM reference")
                elif extra in FLAGS:
                    bit = 1 << FLAGS.index(extra)
                    if flags & bit:
                        raise ValueError("duplicate regulatory flag")
                    flags |= bit
                else:
                    raise ValueError("unknown regulatory flag")
            rule = Rule(start, end, bandwidth, power(match[4], bool(match[5])), flags, reference)
            for code in current:
                if any(old == rule for old, _ in countries[code].rules):
                    raise ValueError("duplicate country rule")
                countries[code].rules.append((rule, number))
                if len(countries[code].rules) > 16:
                    raise ValueError("country exceeds reviewed rule bound")
        except ValueError as error:
            raise ValueError(f"line {number}: {error}") from error
    if not countries or any(not country.rules for country in countries.values()):
        raise ValueError("empty country/catalog")
    if any(set(rows) != set(ACS) for rows in wmms.values()):
        raise ValueError("incomplete WMM group")
    for country in countries.values():
        for channel in range(1, 15):
            center = 2_484_000 if channel == 14 else 2_407_000 + channel * 5000
            matches = [rule for rule, _ in country.rules if rule.bandwidth >= 20_000
                       and rule.start <= center - 10_000 and center + 10_000 <= rule.end]
            if len(matches) > 1:
                raise ValueError("ambiguous complete 2G20 rule: review required")
    return countries, wmms


def generate(source):
    if hashlib.sha256(source).hexdigest() != SHA256:
        raise ValueError("wireless-regdb SHA-256 mismatch; database update requires review")
    countries, wmms = parse(source.decode("utf-8"))
    if len(countries) != 182 or sum(len(country.rules) for country in countries.values()) != 1016:
        raise ValueError("pinned inventory mismatch")
    rules = sorted({rule for country in countries.values() for rule, _ in country.rules})
    indices = {rule: index for index, rule in enumerate(rules)}
    output = ["// SPDX-License-Identifier: ISC", "// Copyright (c) 2008 Luis R. Rodriguez, Johannes Berg, Michael Green.",
              "// Generated by scripts/rtw89_regdb.py; see LICENSES/ISC-wireless-regdb.txt.",
              f"// Source commit {COMMIT}; db.txt SHA-256 {SHA256}.",
              "use super::{Country, DfsRegion, Flags, Rule, RuleRef, Wmm, WmmAc};", "",
              "pub(super) const RULES: &[Rule] = &["]
    for rule in rules:
        wmm = f"Some(&WMM_{rule.wmm})" if rule.wmm else "None"
        output.append(f"    Rule {{ start_khz: {rule.start}, end_khz: {rule.end}, max_bandwidth_khz: {rule.bandwidth}, max_eirp_mbm: {rule.eirp}, flags: Flags({rule.flags}), wmm: {wmm} }},")
    output.extend(["];", "", "pub(super) static COUNTRIES: &[Country] = &["])
    for code, country in sorted(countries.items()):
        refs = ", ".join(f"RuleRef {{ index: {indices[rule]}, source_line: {line} }}" for rule, line in country.rules)
        output.append(f'    Country {{ alpha2: *b"{code}", dfs: DfsRegion::{country.dfs}, source_line: {country.line}, rules: &[{refs}] }},')
    output.extend(["];", ""])
    for name, rows in sorted(wmms.items()):
        output.append(f'const WMM_{name}: Wmm = Wmm {{ name: "{name}", categories: [')
        for ac in ACS:
            minimum, maximum, aifsn, cot = rows[ac]
            output.append(f"    WmmAc {{ cw_min: {minimum}, cw_max: {maximum}, aifsn: {aifsn}, cot_ms: {cot} }},")
        output.extend(["] };", ""])
    return "\n".join(output)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--check", action="store_true")
    args = parser.parse_args()
    generated = generate(SOURCE.read_bytes())
    if args.check:
        if OUTPUT.read_text() != generated:
            raise SystemExit("regulatory_tables.rs differs from the pinned source")
    else:
        OUTPUT.write_text(generated)


if __name__ == "__main__":
    main()
