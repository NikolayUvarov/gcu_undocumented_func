#!/usr/bin/env python3
"""MIND IDL v0.2 generator: layout, limits, schema errors and freshness of the generated bindings."""
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
            parse("variant v { a }\n")


class Bulk(unittest.TestCase):
    """v0.2: records, enums, strings, bytes, lists and result<T, E> in a borrowed buffer."""

    def test_records_enums_and_payload_field(self):
        i = parse("enum e { a, b-c }\nrecord r {\n    name: string<8>,\n    kind: e,\n}\n"
                  "f: func(x: u16, buffer: borrow<memory>, items: list<r, 4>, k: e) -> result<list<u32, 2>, e>;\n")
        self.assertEqual(i.enums["e"].cases, ["a", "b_c"])
        self.assertEqual([n for n, _ in i.records["r"].fields], ["name", "kind"])
        f = i.functions[0]
        self.assertEqual([p.name for p in f.bulk], ["items"])
        # The implicit payload length comes first, then the scalars.
        self.assertEqual((f.payload.word, f.payload.shift), (0, 16))
        self.assertEqual({p.name: (p.word, p.shift) for p in f.scalars}, {"x": (0, 48), "k": (1, 0)})
        self.assertEqual(f.error, "e")

    def test_bulk_needs_a_borrowed_buffer(self):
        for body in ("f: func(s: string<4>);\n", "f: func(b: own<memory>, s: string<4>);\n", "f: func() -> list<u8, 4>;\n"):
            with self.subTest(body=body), self.assertRaises(mind_idl.IdlError):
                parse(body)

    def test_rejected_shapes(self):
        for body in ("record r { l: list<u8, 2> }\nf: func();\n",           # no lists inside records
                     "f: func(b: borrow<memory>, l: list<string<4>, 2>);\n",  # list items: integers, enums, records
                     "f: func() -> result<u8, u8>;\n",                        # the error must be an enum
                     "f: func(b: borrow<memory>, s: string<0>);\n",
                     "record request { a: u8 }\nf: func();\n",              # reserved name
                     "record a { b: b }\nrecord b { x: u8 }\nf: func();\n"):  # declared before use
            with self.subTest(body=body), self.assertRaises(mind_idl.IdlError):
                parse(body)

    def test_sample_bindings_are_fresh(self):
        import tempfile
        with tempfile.TemporaryDirectory() as temp:
            out = Path(temp) / "sample.rs"
            subprocess.run([sys.executable, str(ROOT / "scripts" / "mind_idl.py"), "--one", "tests/idl/sample.wit", str(out)], cwd=ROOT, check=True)
            self.assertEqual(out.read_text(), (ROOT / "tests" / "idl" / "sample.rs").read_text(),
                             "regenerate: python3 scripts/mind_idl.py --one tests/idl/sample.wit tests/idl/sample.rs")

    def test_generated_bindings_are_fresh(self):
        result = subprocess.run([sys.executable, str(ROOT / "scripts" / "mind_idl.py"), "--check"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


class CapabilityResults(unittest.TestCase):
    """A result may be a capability: the reply carries it into a slot the client names."""

    def test_capability_result(self):
        i = parse("enum e { a }\nf: func(x: u32) -> result<borrow<endpoint>, e>;\n")
        f = i.functions[0]
        self.assertEqual((f.ok_handle, f.ok, f.error), (("borrow", "endpoint"), None, "e"))
        rust = mind_idl.generate(i, "t.wit")
        self.assertIn("pub fn f(endpoint: ipc::Endpoint, x: u32, receive: usize) -> Result<core::result::Result<usize, E>>", rust)
        self.assertIn("wire::call_receiving(endpoint, words, None, receive)", rust)
        self.assertIn("crate::dev::cap_info(receive).0 != CAP_KIND_ENDPOINT", rust)
        self.assertIn("wire::reply_cap([0, 0], value, false)", rust)
        moved = mind_idl.generate(parse("enum e { a }\nf: func() -> result<own<memory>, e>;\n"), "t.wit")
        self.assertIn("wire::reply_cap([0, 0], value, true)", moved)

    def test_unknown_capability_kind_rejected(self):
        with self.assertRaises(mind_idl.IdlError):
            parse("enum e { a }\nf: func() -> result<own<page>, e>;\n")


if __name__ == "__main__":
    unittest.main()
