"""Check that export compaction preserves code tokens and never edits sources."""
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from scripts.export_context import (ROOT, blocks, chunk_agent, discover, minify_rust, publish,
                                    rust_tokens, stats)


class RustCompactionTests(unittest.TestCase):
    def test_literals_lifetimes_raw_identifiers_nested_comments_and_operator_gaps(self):
        source = r'''
// a removed comment
/* outer /* inner */ ends */
fn r#type<'a>(value: &'a str) -> &'static str {
    let literals = ("http://host/*literal*/", b"//bytes", c"/*C*/", 'x', '\'', b'\x7f', '\u{41}');
    let raw = r###"quote "## // keep /* keep */
  literal indentation

"###;
    let bytes = br#"//keep"#; let cstr = cr"/*keep*/";
    'r#label: loop { break 'r#label; }
    tokens!(a / * b, a & & b, a > > b, r # name, b "x", 1 . 0, ' separate);
    "continued\
       string"
}
'''
        compact, removed = minify_rust(source, keep_tests=True)
        before, after = rust_tokens(source), rust_tokens(compact)
        self.assertEqual([(t.text, t.kind) for t in before], [(t.text, t.kind) for t in after])
        # Macro punctuation jointness must not change where operators can form.
        def joints(tokens):
            return [(a.text, b.text, b.gap) for a, b in zip(tokens, tokens[1:])
                    if a.kind == b.kind == "punct" and a.text not in "()[]{},;" and b.text not in "()[]{},;"]
        self.assertEqual(joints(before), joints(after))
        self.assertNotIn("a removed comment", compact)
        self.assertIn("  literal indentation\n\n", compact)
        self.assertEqual(removed, 0)

    def test_only_explicit_test_modules_omitted_and_neighbouring_code_kept(self):
        source = '''fn before() {} #[cfg(test)] mod tests {
            const S: &str = "} fake brace {"; fn nested() { if true {} }
        } #[cfg(not(test))] fn after() {}'''
        compact, removed = minify_rust(source)
        self.assertEqual(removed, 1)
        self.assertIn("fn before()", compact)
        self.assertIn("fn after()", compact)
        self.assertNotIn("fake brace", compact)
        kept, removed = minify_rust(source, keep_tests=True)
        self.assertIn("fake brace", kept)
        self.assertEqual(removed, 0)

    def test_malformed_literals_or_comments_fail_instead_of_silently_truncating(self):
        for source in ['"unterminated', 'r##"not closed"#', '/* outer /* inner */']:
            with self.subTest(source=source), self.assertRaises(ValueError):
                minify_rust(source)

    def test_all_real_rust_token_streams_and_literals_survive(self):
        files = discover(ROOT, {"code"}, [], ROOT / "code_handoff")
        self.assertGreaterEqual(len(files), 21)
        for item in files:
            with self.subTest(path=item["path"]):
                compact, _ = minify_rust(item["text"], keep_tests=True)
                self.assertEqual([(t.text, t.kind) for t in rust_tokens(item["text"])],
                                 [(t.text, t.kind) for t in rust_tokens(compact)])


class ExportTests(unittest.TestCase):
    def test_scope_hidden_configs_untracked_files_dedup_and_source_immutability(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            data = {"kernel/src/main.rs": b"fn main() { /* exact source */ }\r\n",
                    "kernel/.cargo/config.toml": b'[build]\ntarget="x86_64-unknown-none"\n',
                    "app/.cargo/config.toml": b'[build]\ntarget="x86_64-unknown-none"\n',
                    "02_build.sh": b'#!/bin/bash\ncat <<\'END\'\n # literal\n\nEND\n',
                    "scripts/helper.py": b'def f():\n    return "  keep  "\n',
                    "tests/a.rs": b"#[test] fn test() {}", "README.md": b"current docs",
                    "legacy/old.rs": b"old", "patches_seq/patch.sh": b"patch",
                    "code_context.txt": b"old dump", "kernel/target/a.rs": b"generated",
                    "kernel/Cargo.lock": b"lockfile", ".env": b"not source"}
            for name, value in data.items():
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(value)
            (root / "common").mkdir()
            (root / "common/linked.rs").symlink_to(root / "legacy/old.rs")
            original_hashes = {name: hashlib.sha256(value).hexdigest() for name, value in data.items()}
            args = [sys.executable, str(ROOT / "scripts/export_context.py"), "--root", str(root)]
            subprocess.run(args, check=True, capture_output=True)
            output = root / "code_handoff"
            manifest = json.loads((output / "manifest.json").read_text())
            names = {f["path"] for f in manifest["files"]}
            self.assertEqual(names, {"kernel/src/main.rs", "kernel/.cargo/config.toml",
                                    "app/.cargo/config.toml", "02_build.sh", "scripts/helper.py"})
            self.assertIn(data["kernel/src/main.rs"], (output / "code.txt").read_bytes())
            self.assertIn(data["02_build.sh"], (output / "build.txt").read_bytes())
            self.assertIn("[identical source: app/.cargo/config.toml]", (output / "build.txt").read_text())
            self.assertEqual(original_hashes, {name: hashlib.sha256((root / name).read_bytes()).hexdigest() for name in data})
            snapshot = {p.relative_to(output): p.read_bytes() for p in output.rglob("*") if p.is_file()}
            subprocess.run(args, check=True, capture_output=True)
            self.assertEqual(snapshot, {p.relative_to(output): p.read_bytes() for p in output.rglob("*") if p.is_file()})
            failed = subprocess.run(args + ["--include", "missing"], capture_output=True)
            self.assertNotEqual(failed.returncode, 0)
            self.assertEqual(snapshot, {p.relative_to(output): p.read_bytes() for p in output.rglob("*") if p.is_file()})

    def test_chunks_are_bounded_and_reconstruct_all_bodies_without_loss(self):
        entries = [("a.rs", "fn a() {\n" + "  value();\n" * 40 + "}\n"),
                   ("b.rs", 'r#"' + "я" * 800 + '"#;\n')]
        header = "Agent view\n"
        parts = chunk_agent(header, entries, 160)
        self.assertTrue(all(len(p) <= 160 for p in parts))
        collected = {name: "" for name, _ in entries}
        pattern = re.compile(r"^@@ (\S+) \[chars (\d+)\+: compact body\]\n", re.M)
        for part in parts:
            content = part[len(header):]
            matches = list(pattern.finditer(content))
            for index, match in enumerate(matches):
                name, offset = match.group(1), int(match.group(2))
                end = matches[index + 1].start() if index + 1 < len(matches) else len(content)
                self.assertEqual(offset, len(collected[name]))
                collected[name] += content[match.end():end]
        self.assertEqual(collected, dict(entries))

    def test_stale_generated_files_cleaned_without_touching_other_files(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            first = {"tests.txt": "old", "parts/agent-003.txt": "old"}
            publish(root, {**first, "manifest.json": json.dumps({"generator": "mind-core-context-v1", "outputs": first})})
            (root / "notes.txt").write_text("user notes")
            publish(root, {"agent.txt": "new", "manifest.json": "{}"})
            self.assertFalse((root / "tests.txt").exists())
            self.assertFalse((root / "parts/agent-003.txt").exists())
            self.assertEqual((root / "notes.txt").read_text(), "user notes")

    def test_token_estimate_is_character_based_not_claimed_exact(self):
        self.assertEqual(stats("абcd"), {"bytes": 6, "characters": 4, "estimated_tokens": 1})


if __name__ == "__main__":
    unittest.main()
