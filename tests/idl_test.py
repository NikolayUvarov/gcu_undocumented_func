#!/usr/bin/env python3
"""MIND IDL v0 generator: layout, limits, schema errors and freshness of the generated bindings."""
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import mind_idl  # noqa: E402

HEADER = "package mind:t@1.0.0;\ninterface t {\n"
SELFTEST = """/// Every v0 feature.
package mind:selftest@2.1.0;

interface selftest {
    /// No parameters, no result.
    ping: func();
    flags: func(a: bool, b: u8, c: u16, d: u32, e: u32) -> bool;
    wide: func(a: u64) -> u64;
    give: func(buffer: own<memory>, length: u32) -> option<u16>;
    lend: func(reply-to: borrow<endpoint>);
}
"""


def parse(body):
    return mind_idl.parse(HEADER + body + "}\n")


class Layout(unittest.TestCase):
    def test_fields_pack_without_straddling(self):
        f = parse("f: func(a: u32, b: u16, c: bool, d: u32, own-buf: own<memory>) -> u64;\n").functions[0]
        placed = {p.name: (p.word, p.shift) for p in f.params if not p.handle}
        self.assertEqual(placed, {"a": (0, 16), "b": (0, 48), "c": (1, 0), "d": (1, 1)})
        self.assertEqual(f.handle.name, "own_buf")
        self.assertEqual(f.result_field.shift, 0)  # a 64-bit result starts word 1
        self.assertEqual(f.result_field.word, 1)

    def test_word_overflow_rejected(self):
        with self.assertRaises(mind_idl.IdlError):
            parse("f: func(a: u64, b: u64) -> u8;\n")  # 64-bit field cannot start in word 0 after the header

    def test_one_capability_per_message(self):
        with self.assertRaises(mind_idl.IdlError):
            parse("f: func(a: own<memory>, b: borrow<endpoint>);\n")

    def test_unknown_types_rejected(self):
        for body in ("f: func(a: string);\n", "f: func(a: own<socket>);\n", "f: func() -> option<own<memory>>;\n"):
            with self.subTest(body=body), self.assertRaises(mind_idl.IdlError):
                parse(body)

    def test_version_and_syntax(self):
        with self.assertRaises(mind_idl.IdlError):
            mind_idl.parse("package mind:t@0.1.0;\ninterface t {\nf: func();\n}\n")
        with self.assertRaises(mind_idl.IdlError):
            parse("record r { a: u8 }\n")

    def test_every_feature_compiles(self):
        # Generated code for all supported types, capabilities and results builds inside a copy of libmind.
        with tempfile.TemporaryDirectory() as tmp:
            for part in ("libmind", "common"):
                shutil.copytree(ROOT / part, Path(tmp) / part, ignore=shutil.ignore_patterns("target"))
            (Path(tmp) / "idl").mkdir()
            shutil.copy(ROOT / "idl" / "rtc.wit", Path(tmp) / "idl")
            (Path(tmp) / "idl" / "selftest.wit").write_text(SELFTEST)
            subprocess.run([sys.executable, str(ROOT / "scripts" / "mind_idl.py"), "--root", tmp], check=True, capture_output=True)
            build = subprocess.run(["cargo", "+nightly", "build", "--release", "--target", "x86_64-unknown-none"], cwd=Path(tmp) / "libmind", capture_output=True, text=True)
            self.assertEqual(build.returncode, 0, build.stderr[-3000:])
            self.assertNotIn("src/idl/", build.stderr, "generated code must build without warnings")

    def test_generated_bindings_are_fresh(self):
        result = subprocess.run([sys.executable, str(ROOT / "scripts" / "mind_idl.py"), "--check"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
