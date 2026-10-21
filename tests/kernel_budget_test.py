#!/usr/bin/env python3
"""Host test of scripts/kernel_budget.py (850-KRN-0145): what counts as a line of the kernel, the areas, and the
check above and at the budget (a made-up tree)."""
import importlib.util, json, pathlib, tempfile

ROOT = pathlib.Path(__file__).resolve().parent.parent
spec = importlib.util.spec_from_file_location("kernel_budget", ROOT / "scripts/kernel_budget.py")
kb = importlib.util.module_from_spec(spec)
spec.loader.exec_module(kb)

SOURCE = """//! A module.
// A comment.

/// A function.
fn f() {
    /* one line */
    /* a block
       of three
       lines */
    let x = 1; // a comment after code counts as code
}
"""


def tree(base, budget):
    src = pathlib.Path(base) / "kernel/src"
    for name in ("main.rs", "arch/mod.rs", "arch/x86_64/cpu.rs", "arch/aarch64/cpu.rs"):
        (src / name).parent.mkdir(parents=True, exist_ok=True)
        (src / name).write_text(SOURCE)
    measured = {"date": "2026-10-21", "commit": "abc1234", "lines": 12}
    (pathlib.Path(base) / kb.BUDGET).write_text(json.dumps({"budget": budget, "measured": measured}))


def test_code_lines():
    assert kb.code_lines(SOURCE) == (3, 11), kb.code_lines(SOURCE)
    assert kb.code_lines("") == (0, 0)


def test_areas_and_budget():
    with tempfile.TemporaryDirectory() as base:
        tree(base, 12)
        assert kb.count(base) == {"common": [6, 22], "aarch64": [3, 11], "x86_64": [3, 11]}, kb.count(base)  # arch/mod.rs is common
        report, within = kb.check(base)
        assert within and "12 lines" in report and "(44 with them)" in report and "budget 12" in report, report
        tree(base, 11)
        report, within = kb.check(base)
        assert not within and "above its budget by 1" in report, report


def test_the_kernel_is_within_its_budget():
    report, within = kb.check(ROOT)
    assert within, report


if __name__ == "__main__":
    for name, test in list(globals().items()):
        if name.startswith("test_"):
            test()
    print("kernel_budget_test: all passed")
