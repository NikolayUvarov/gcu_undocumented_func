#!/usr/bin/env python3
"""MIND IDL v0 generator: layout, limits, schema errors and freshness of the generated bindings."""
import subprocess
import sys
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
sys.path.insert(0, str(ROOT / "scripts"))
import mind_idl  # noqa: E402

HEADER = "package mind:t@1.0.0;\ninterface t {\n"


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

    def test_generated_bindings_are_fresh(self):
        result = subprocess.run([sys.executable, str(ROOT / "scripts" / "mind_idl.py"), "--check"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


if __name__ == "__main__":
    unittest.main()
