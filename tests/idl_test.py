#!/usr/bin/env python3
"""MIND IDL generator: layout, limits, schema errors, the extensions (enums, bytes, capability results) and freshness
of the generated bindings."""
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
SELFTEST = """/// Every v0.2 feature.
package mind:selftest@2.1.0;

interface selftest {
    /// A record with every field kind.
    record entry {
        name: string<32>,
        size: u64,
        directory: bool,
        tags: list<u8, 4>,
    }
    record pair { left: entry, right: u16 }
    /// No parameters, no result.
    ping: func();
    flags: func(a: bool, b: u8, c: u16, d: u32, e: u32) -> bool;
    wide: func(a: u64) -> u64;
    give: func(buffer: own<memory>, length: u32) -> option<u16>;
    lend: func(reply-to: borrow<endpoint>);
    fail: func(code: u32) -> result<u32, error-code>;
    nothing: func(code: u32) -> result<_, error-code>;
    open: func(path: string<255>, write: bool) -> result<u64, error-code>;
    list: func(path: string<255>, limit: u16) -> list<entry, 32>;
    find: func(names: list<string<16>, 8>) -> option<pair>;
    put: func(item: entry, more: list<pair, 2>);
    name: func(id: u32) -> string<64>;
}
"""


def parse(body):
    return mind_idl.parse(HEADER + body + "}\n")


class Layout(unittest.TestCase):
    def test_fields_pack_without_straddling(self):
        f = parse("f: func(a: u32, b: u16, c: bool, d: u32, own-buf: own<memory>) -> u64;\n").functions[0]
        placed = {p.name: (p.word, p.shift) for p in f.params if p.type[0] == "int"}
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
        for body in ("f: func(a: string);\n", "f: func(a: own<socket>);\n", "f: func() -> option<own<memory>>;\n", "f: func(a: list<u8>);\n"):
            with self.subTest(body=body), self.assertRaises(mind_idl.IdlError):
                parse(body)

    def test_buffer_calls(self):
        interface = parse("record e { a: string<4>, b: u16 }\nf: func(x: list<e, 3>) -> string<10>;\ng: func(y: u8);\n")
        f, g = interface.functions
        self.assertTrue(f.buffered and not g.buffered)
        self.assertEqual((f.request_max, f.reply_max), (2 + 3 * (6 + 2), 12))
        for body in ("f: func(x: string<4>, h: own<memory>);\n",  # a buffer call carries no other capability
                     "f: func() -> list<u8, 70000>;\n",            # more than 64 KiB
                     "f: func(x: list<own<memory>, 2>);\n",
                     "record r { h: own<memory> }\nf: func(x: r);\n",
                     "f: func(x: unknown);\n"):
            with self.subTest(body=body), self.assertRaises(mind_idl.IdlError):
                parse(body)

    def test_version_and_syntax(self):
        with self.assertRaises(mind_idl.IdlError):
            mind_idl.parse("package mind:t@0.1.0;\ninterface t {\nf: func();\n}\n")
        with self.assertRaises(mind_idl.IdlError):
            parse("record r { a: u8 }\n")

    def test_generated_sample_is_fresh(self):
        # tests/idl/sample.rs (the host loopback test, tests/idl_host.rs) is generated from tests/idl/sample.wit.
        with tempfile.TemporaryDirectory() as tmp:
            target = Path(tmp) / "sample.rs"
            subprocess.run([sys.executable, str(ROOT / "scripts" / "mind_idl.py"), "--one", "tests/idl/sample.wit", str(target)], cwd=ROOT, check=True)
            self.assertEqual(target.read_text(), (ROOT / "tests" / "idl" / "sample.rs").read_text(), "run: scripts/mind_idl.py --one tests/idl/sample.wit tests/idl/sample.rs")

    def test_every_feature_compiles(self):
        # Generated code for all supported types, capabilities and results builds inside a copy of libmind.
        with tempfile.TemporaryDirectory() as tmp:
            for part in ("libmind", "common"):
                shutil.copytree(ROOT / part, Path(tmp) / part, ignore=shutil.ignore_patterns("target"))
            shutil.copy(ROOT / "rust-toolchain.toml", tmp)  # the pinned toolchain, not the user's default
            (Path(tmp) / "idl").mkdir()
            for wit in (ROOT / "idl").glob("*.wit"):
                shutil.copy(wit, Path(tmp) / "idl")
            (Path(tmp) / "idl" / "selftest.wit").write_text(SELFTEST)
            shutil.copy(ROOT / "tests" / "idl" / "sample.wit", Path(tmp) / "idl")  # the extensions
            subprocess.run([sys.executable, str(ROOT / "scripts" / "mind_idl.py"), "--root", tmp], check=True, capture_output=True)
            build = subprocess.run(["cargo", "build", "--release", "--target", "x86_64-unknown-none"], cwd=Path(tmp) / "libmind", capture_output=True, text=True)
            self.assertEqual(build.returncode, 0, build.stderr[-3000:])
            self.assertNotIn("src/idl/", build.stderr, "generated code must build without warnings")

    def test_generated_bindings_are_fresh(self):
        result = subprocess.run([sys.executable, str(ROOT / "scripts" / "mind_idl.py"), "--check"], capture_output=True, text=True)
        self.assertEqual(result.returncode, 0, result.stderr)


class Extensions(unittest.TestCase):
    """Enums, bytes and capability results: a minor extension of v0.2 (existing interfaces generate unchanged)."""

    def test_enums(self):
        interface = parse("enum color { red, green, blue }\nrecord r { c: color }\nf: func(c: color, x: u8) -> result<color, color>;\ng: func(x: r) -> list<color, 3>;\n")
        f, g = interface.functions
        self.assertFalse(f.buffered)
        placed = {p.name: (p.word, p.shift) for p in f.params}
        self.assertEqual(placed, {"c": (0, 16), "x": (0, 24)})  # an enum is an 8-bit field
        self.assertEqual(f.result, (("enum", "color"), False, False, "color"))
        self.assertTrue(g.buffered)
        self.assertEqual((g.request_max, g.reply_max), (1, 2 + 3))
        multi = parse("enum e {\n    a,\n    b\n}\nf: func(x: e);\n")
        self.assertEqual(multi.records["e"].cases, ["a", "b"])
        for body in ("enum e { }\nf: func();\n",                       # no cases
                     "enum e { a, a }\nf: func();\n",                  # duplicate
                     "enum e { A }\nf: func();\n",                     # bad case name
                     "enum e { a }\nenum e { b }\nf: func();\n",      # declared twice
                     "f: func() -> result<u8, nothing>;\n",            # the error is error-code or an enum
                     "record endpoint { a: u8 }\nf: func();\n",        # collides with the generated code
                     "enum e { a }\nf: func(request: e);\n"):
            with self.subTest(body=body), self.assertRaises(mind_idl.IdlError):
                parse(body)

    def test_error_enum_renames_the_system_error(self):
        text = mind_idl.generate(parse("enum error { a }\nf: func() -> result<u8, error>;\ng: func() -> result<u8, error-code>;\n"), "t.wit")
        self.assertIn("use crate::sys::{Error as SysError, Result};", text)
        self.assertIn("-> Result<core::result::Result<u8, Error>>", text)
        self.assertIn("pub fn reply_g(call: Call, value: Result<u8>)", text)

    def test_bytes(self):
        f = parse("f: func(a: u32, data: bytes<100>) -> result<bytes<200>, error-code>;\n").functions[0]
        self.assertTrue(f.buffered)
        self.assertEqual((f.request_max, f.reply_max), (4 + 2 + 100, 2 + 200))
        for body in ("f: func(a: bytes<0>);\n", "f: func(a: bytes<65536>);\n", "record r { b: bytes<4> }\nf: func(x: r);\n",
                     "f: func(a: list<bytes<4>, 2>);\n", "f: func(buffer: bytes<4>);\n"):
            with self.subTest(body=body), self.assertRaises(mind_idl.IdlError):
                parse(body)
        text = mind_idl.generate(parse("f: func(data: bytes<8>) -> bytes<8>;\n"), "t.wit")
        self.assertIn("pub fn f(endpoint: Endpoint, data: &[u8], out: &mut [u8]) -> Result<usize>", text)
        self.assertIn("pub fn decode<'a>(request: &Received, cap: usize, scratch: &'a mut [u8; REQUEST_MAX])", text)

    def test_capability_results(self):
        f = parse("enum e { a }\nf: func(id: u32) -> result<borrow<endpoint>, e>;\n").functions[1 - 1]
        self.assertFalse(f.buffered)
        self.assertIsNone(f.result_field)
        text = mind_idl.generate(parse("f: func(id: u32) -> own<endpoint>;\n"), "t.wit")
        self.assertIn("pub fn f(endpoint: Endpoint, id: u32, receive: usize) -> Result<()>", text)
        self.assertIn("wire::finish_cap(call, value, true)", text)
        for body in ("f: func(s: string<4>) -> result<borrow<endpoint>, error-code>;\n",  # a buffer call has no other capability
                     "f: func() -> option<own<endpoint>>;\n"):
            with self.subTest(body=body), self.assertRaises(mind_idl.IdlError):
                parse(body)


if __name__ == "__main__":
    unittest.main()
