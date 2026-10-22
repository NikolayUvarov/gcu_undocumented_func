#!/usr/bin/env python3
"""The kernel's size against its budget (850-KRN-0145, MC-1.1–1.3): the lines of Rust in kernel/src without blank
lines and comments, the common part and each architecture's apart, checked against kernel/size-budget.json.

Usage:
    kernel_budget.py [--root DIR]

    --root DIR   the repository (default: this script's)

Prints the count and the budget. Exit status: 0 at or below the budget, 1 above it. Raising the budget is a commit of
its own that names the mechanism needing the room and why it cannot live in user space (MC-1.2).
"""
import argparse, json, pathlib, sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
BUDGET = "kernel/size-budget.json"


def code_lines(text):
    """The lines that are neither blank nor comments; physical lines too."""
    code, block = 0, False
    lines = text.splitlines()
    for line in lines:
        s = line.strip()
        if block:
            block = "*/" not in s
            continue
        if not s or s.startswith("//"):
            continue
        if s.startswith("/*"):
            block = "*/" not in s
            continue
        code += 1
    return code, len(lines)


def count(root):
    """{area: [code, physical]}: "common", then each directory of kernel/src/arch."""
    src = pathlib.Path(root) / "kernel/src"
    areas = {"common": [0, 0]}
    for path in sorted(src.rglob("*.rs")):
        parts = path.relative_to(src).parts
        area = parts[1] if parts[0] == "arch" and len(parts) > 2 else "common"
        code, physical = code_lines(path.read_text())
        totals = areas.setdefault(area, [0, 0])
        totals[0] += code
        totals[1] += physical
    return areas


def check(root):
    """The report and whether the count is within the budget."""
    budget = json.loads((pathlib.Path(root) / BUDGET).read_text())
    areas = count(root)
    code = sum(a[0] for a in areas.values())
    physical = sum(a[1] for a in areas.values())
    measured = budget["measured"]
    parts = ", ".join(f"{name} {a[0]}" for name, a in areas.items())
    report = (f"kernel size (850-KRN-0145): {code} lines of Rust without blank lines and comments ({physical} with them): "
              f"{parts}; budget {budget['budget']} (measured {measured['lines']} at {measured['commit']}, {measured['date']})")
    if code > budget["budget"]:
        report += (f"\nFAIL: the kernel is above its budget by {code - budget['budget']}; move the mechanism to user space, or"
                   f" raise {BUDGET} in a commit of its own that names it and why it needs the kernel (MC-1.2)")
    return report, code <= budget["budget"]


def main():
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--root", default=ROOT)
    report, within = check(parser.parse_args().root)
    print(report)
    return 0 if within else 1


if __name__ == "__main__":
    sys.exit(main())
