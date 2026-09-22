#!/usr/bin/env python3
"""Export selected live MIND CORE sources and a compact, explicitly scoped agent view."""
import argparse
from dataclasses import dataclass
import fnmatch
import hashlib
import json
import math
import os
from pathlib import Path
import re
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]
CODE_ROOTS = {"kernel", "bootloader", "app", "app2", "clock", "dzen-clock", "common"}
SKIP = {"target", "build", "dist", "out", "code_handoff", "code-handoff", "usb_root", "legacy", "patches_seq",
        "issues", "issues-done", "knowledge", "node_modules", "vendor", "__pycache__"}
GROUPS = ("code", "build", "tests", "docs")
IDENT = re.compile(r"(?:r#)?(?:[^\W\d]|_)\w*", re.UNICODE)
RAW = re.compile(r'(?:br|cr|r)(#{0,255})"')
CHAR = re.compile(r"(?:b)?'(?:\\(?:u\{[0-9a-fA-F_]+\}|x[0-9a-fA-F]{2}|[^\r\n])|[^'\\\r\n])'")
MARKER = "@@ "


@dataclass
class Token:
    text: str
    kind: str
    gap: bool


def rust_tokens(source):
    """Small lossless lexer for compaction, not a Rust parser. Literals stay opaque.

    Comments become whitespace; punctuation remains individual tokens so we
    can retain separated operators instead of turning `/ *` into a comment.
    """
    tokens, i, gap = [], 0, False
    while i < len(source):
        if source[i].isspace() or (i == 0 and source[i] == '\ufeff'):
            gap = True
            i += 1
            continue
        if source.startswith("//", i):
            end = source.find("\n", i)
            i = len(source) if end < 0 else end
            gap = True
            continue
        if source.startswith("/*", i):
            depth, i = 1, i + 2
            while depth and i < len(source):
                if source.startswith("/*", i):
                    depth, i = depth + 1, i + 2
                elif source.startswith("*/", i):
                    depth, i = depth - 1, i + 2
                else:
                    i += 1
            if depth:
                raise ValueError("unterminated Rust block comment")
            gap = True
            continue
        start, kind = i, "punct"
        raw = RAW.match(source, i)
        char = CHAR.match(source, i)
        word = IDENT.match(source, i)
        if raw:
            terminator = '"' + raw.group(1)
            end = source.find(terminator, raw.end())
            if end < 0:
                raise ValueError("unterminated Rust raw string")
            i, kind = end + len(terminator), "literal"
        elif source[i] == '"' or source.startswith(('b"', 'c"'), i):
            i += 1 if source[i] == '"' else 2
            while i < len(source):
                if source[i] == "\\":
                    i += 2
                elif source[i] == '"':
                    i += 1
                    break
                else:
                    i += 1
            else:
                raise ValueError("unterminated Rust string")
            kind = "literal"
        elif char:
            i, kind = char.end(), "literal"
        elif source[i] == "'" and IDENT.match(source, i + 1):
            i, kind = IDENT.match(source, i + 1).end(), "word"
        elif word:
            i, kind = word.end(), "word"
        elif source[i].isdigit():
            i += 1
            while i < len(source) and (source[i].isalnum() or source[i] == "_"):
                i += 1
            kind = "number"
        else:
            i += 1
        tokens.append(Token(source[start:i], kind, gap))
        gap = False
    return tokens


def omit_test_modules(tokens):
    """Only the unambiguous #[cfg(test)] mod NAME { ... } form is omitted."""
    output, removed, i = [], 0, 0
    prefix = ["#", "[", "cfg", "(", "test", ")", "]", "mod"]
    while i < len(tokens):
        if ([t.text for t in tokens[i:i + 8]] == prefix and i + 9 < len(tokens)
                and tokens[i + 8].kind == "word" and tokens[i + 9].text == "{"):
            depth, end = 1, i + 10
            while end < len(tokens) and depth:
                if tokens[end].text == "{":
                    depth += 1
                elif tokens[end].text == "}":
                    depth -= 1
                end += 1
            if depth:
                raise ValueError("unclosed #[cfg(test)] module")
            i, removed = end, removed + 1
        else:
            output.append(tokens[i])
            i += 1
    return output, removed


def needs_space(left, right):
    if not right.gap:
        return False
    if left.kind != "punct" and right.kind != "punct":
        return True
    # Do not introduce raw identifiers, literal prefixes, lifetimes or floats.
    if left.text in ("#", "'") or right.text in ("#", "'"):
        return True
    if (left.kind == "number" and right.text == ".") or (left.text == "." and right.kind == "number"):
        return True
    delimiters = set("()[]{},;")
    return (left.kind == right.kind == "punct" and left.text not in delimiters
            and right.text not in delimiters)


def minify_rust(source, keep_tests=False):
    tokens = rust_tokens(source)
    tokens, removed = (tokens, 0) if keep_tests else omit_test_modules(tokens)
    output, previous = [], None
    for token in tokens:
        if previous:
            if previous.text in (";", "}") and token.text not in (";", ",", ")", "]", "}", "."):
                output.append("\n")
            elif needs_space(previous, token):
                output.append(" ")
        output.append(token.text)
        previous = token
    return "".join(output) + "\n", removed


def classify(path):
    parts, suffix = path.parts, path.suffix
    if parts[0] in CODE_ROOTS:
        if suffix == ".rs":
            return "code"
        if suffix in (".toml", ".ld") or path.name == "config":
            return "build"
    if parts[0] == "tests" and suffix in (".rs", ".py", ".ps1", ".sh"):
        return "tests"
    if parts[0] == "scripts" and suffix in (".py", ".cs", ".ps1", ".sh"):
        return "build"
    if len(parts) == 1:
        if suffix in (".sh", ".bat", ".ps1", ".toml", ".ld"):
            return "build"
        if path.name == "README.md":
            return "docs"
    if parts[0] == ".cargo" and suffix == ".toml":
        return "build"
    return None


def discover(root, enabled, patterns, out_dir):
    found, matched = [], set()
    for directory, dirs, names in os.walk(root):
        dirs[:] = sorted(d for d in dirs if d not in SKIP and (not d.startswith(".") or d == ".cargo")
                         and not (Path(directory) / d).is_symlink()
                         and (Path(directory) / d).resolve() != out_dir)
        for name in sorted(names):
            path = Path(directory) / name
            relative = path.relative_to(root)
            group = classify(relative)
            if group not in enabled or path.is_symlink():
                continue
            rel = relative.as_posix()
            matching = [p for p in patterns if fnmatch.fnmatchcase(rel, p) or rel == p.rstrip("/")
                        or rel.startswith(p.rstrip("/") + "/")]
            if patterns and not matching:
                continue
            matched.update(matching)
            data = path.read_bytes()
            found.append({"path": rel, "group": group, "text": data.decode("utf-8"),
                          "bytes": len(data), "sha256": hashlib.sha256(data).hexdigest()})
    if set(patterns) - matched:
        raise ValueError("Не найдены пути в выбранных группах: " + ", ".join(sorted(set(patterns) - matched)))
    if not found:
        raise ValueError("Не найдены актуальные исходники.")
    return sorted(found, key=lambda item: item["path"])


def blocks(files, compact=False, keep_tests=False):
    result, seen = [], {}
    for item in files:
        body, omitted = item["text"], 0
        if compact and item["path"].endswith(".rs"):
            try:
                body, omitted = minify_rust(body, keep_tests)
            except ValueError as error:
                raise ValueError(f"{item['path']}: {error}") from error
        item["agent_test_modules_omitted"] = omitted if compact else item.get("agent_test_modules_omitted", 0)
        # Deduplicate only byte-identical source files, never "similar" code.
        if item["sha256"] in seen:
            result.append((item["path"], f"[identical source: {seen[item['sha256']]}]\n"))
        else:
            seen[item["sha256"]] = item["path"]
            result.append((item["path"], body if body.endswith("\n") else body + "\n"))
    return result


def render(header, entries):
    return header + "".join(f"{MARKER}{path}\n{body}" for path, body in entries)


def chunk_agent(header, entries, limit):
    """Bounded copy/paste parts with file + character offsets, no silent truncation.

    Prefer line boundaries; an unusually long literal/line is split explicitly.
    Offsets refer to the compact body, not original source line numbers.
    """
    if not limit:
        return []
    chunks, current = [], header
    for path, body in entries:
        offset = 0
        while offset < len(body):
            marker = f"{MARKER}{path} [chars {offset}+: compact body]\n"
            room = limit - len(current) - len(marker)
            if room < 1:
                if current == header:
                    raise ValueError("--chunk-chars слишком мал для заголовка и имени файла")
                chunks.append(current)
                current = header
                continue
            take = min(room, len(body) - offset)
            if take < len(body) - offset:
                newline = body.rfind("\n", offset, offset + take)
                if newline >= offset:
                    take = newline + 1 - offset
            current += marker + body[offset:offset + take]
            offset += take
            if offset < len(body):
                chunks.append(current)
                current = header
    if current != header:
        chunks.append(current)
    return chunks


def stats(text):
    return {"bytes": len(text.encode("utf-8")), "characters": len(text),
            "estimated_tokens": math.ceil(len(text) / 4)}


def publish(directory, outputs):
    directory.mkdir(parents=True, exist_ok=True)
    old_manifest = directory / "manifest.json"
    previous = {}
    if old_manifest.is_file():
        try:
            previous = json.loads(old_manifest.read_text(encoding="utf-8"))
        except (ValueError, OSError):
            pass
    for name, content in outputs.items():
        target = directory / name
        target.parent.mkdir(parents=True, exist_ok=True)
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", newline="", dir=target.parent,
                                         prefix=".context-", suffix=".tmp", delete=False) as stream:
            temporary = Path(stream.name)
            stream.write(content)
        try:
            temporary.replace(target)
        finally:
            temporary.unlink(missing_ok=True)
    if previous.get("generator") == "mind-core-context-v1":
        for name in previous.get("outputs", {}):
            if name not in outputs and re.fullmatch(r"(?:code|build|tests|docs|agent|index)\.txt|parts/agent-\d{3,}\.txt", name):
                (directory / name).unlink(missing_ok=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--root", type=Path, default=ROOT, help="корень проекта")
    parser.add_argument("--out-dir", type=Path, help="каталог результатов; по умолчанию code_handoff")
    parser.add_argument("--include", action="append", default=[], help="путь/каталог/glob; можно повторять")
    parser.add_argument("--with-tests", action="store_true", help="добавить tests.txt")
    parser.add_argument("--with-docs", action="store_true", help="добавить docs.txt с актуальным README")
    parser.add_argument("--agent-groups", default="code", help="группы для agent.txt: code,build,tests,docs")
    parser.add_argument("--keep-tests-in-agent", action="store_true", help="сохранить cfg(test) модули в Rust")
    parser.add_argument("--chunk-chars", type=int, default=16000, help="лимит символов каждой части; 0 отключает части")
    args = parser.parse_args()
    root = args.root.resolve(strict=True)
    out_dir = (args.out_dir or root / "code_handoff").resolve()
    if not root.is_dir() or out_dir == root:
        raise ValueError("Нужен каталог проекта и отдельный каталог результатов.")
    if args.chunk_chars < 0 or (args.chunk_chars and args.chunk_chars < 1000):
        raise ValueError("--chunk-chars должен быть 0 или не меньше 1000")
    enabled = {"code", "build"} | ({"tests"} if args.with_tests else set()) | ({"docs"} if args.with_docs else set())
    agent_groups = args.agent_groups.split(",")
    if not agent_groups or set(agent_groups) - enabled:
        raise ValueError("Неизвестная/отключённая группа agent; включите --with-tests/--with-docs при необходимости.")
    files = discover(root, enabled, args.include, out_dir)
    outputs = {}
    for group in GROUPS:
        subset = [f for f in files if f["group"] == group]
        if subset:
            outputs[f"{group}.txt"] = render(f"MIND CORE {group}: original contents; @@ path; identical files referenced.\n", blocks(subset))
    full_bundle_names = ", ".join(outputs)
    agent_files = [f for f in files if f["group"] in agent_groups]
    if not agent_files:
        raise ValueError("Для agent.txt не выбрано файлов; измените --agent-groups или --include.")
    entries = blocks(agent_files, compact=True, keep_tests=args.keep_tests_in_agent)
    omitted = sum(f.get("agent_test_modules_omitted", 0) for f in agent_files)
    scope = ",".join(agent_groups)
    header = (f"MIND CORE agent view: {scope}; {len(agent_files)} files. Rust comments removed; literals/ASM preserved; "
              f"cfg(test) modules omitted: {omitted}. Other file types unchanged. @@ = file; identical = reuse. "
              f"Scoped excerpt, not a patch. Full bundles: {full_bundle_names}; inventory: index.txt.\n")
    outputs["agent.txt"] = render(header, entries)
    chunks = chunk_agent(header + "Parts: concatenate bodies by file/character offsets; a part may end mid-expression.\n", entries, args.chunk_chars)
    for number, chunk in enumerate(chunks, 1):
        outputs[f"parts/agent-{number:03}.txt"] = chunk
    original_bytes = sum(f["bytes"] for f in files)
    agent_input_bytes = sum(f["bytes"] for f in agent_files)
    index = ["MIND CORE context export", f"Full bundles: {full_bundle_names}.",
             "Paste agent.txt OR parts/agent-*.txt in order, not both. Select fewer files with --include.",
             "Excluded: history, issues, knowledge snapshots, lockfiles, build outputs, old context dumps, symlinks.",
             "Rust agent view omits comments and (by default) cfg(test) modules; no runtime bodies/ASM/data elided.",
             "Token counts are characters/4 estimates, NOT counts for a particular model/tokenizer.",
             f"Selection: {args.include or 'all enabled live sources'}; agent groups: {scope}", "", "Files:"]
    index.extend(f"{f['group']:5} {f['path']} ({f['bytes']} bytes)" for f in files)
    index += ["", "Outputs (bytes / estimated tokens):"]
    index.extend(f"{name}: {stats(text)['bytes']} / ~{stats(text)['estimated_tokens']}" for name, text in outputs.items())
    outputs["index.txt"] = "\n".join(index) + "\n"
    manifest = {"generator": "mind-core-context-v1", "token_estimate": "ceil(characters / 4); not model-specific",
                "selection": args.include, "agent_groups": agent_groups, "source_bytes": original_bytes,
                "agent_input_bytes": agent_input_bytes, "files": [{k: v for k, v in f.items() if k != "text"} for f in files],
                "outputs": {name: stats(text) for name, text in outputs.items()}}
    outputs["manifest.json"] = json.dumps(manifest, ensure_ascii=False, indent=2) + "\n"
    publish(out_dir, outputs)
    print(f"Готово: {out_dir}\nФайлов: {len(files)}; выбрано {original_bytes:,} байт исходников.")
    for name in ("code.txt", "build.txt", "tests.txt", "docs.txt", "agent.txt"):
        if name in outputs:
            info = stats(outputs[name])
            print(f"{name}: {info['bytes']:,} байт; ~{info['estimated_tokens']:,} токенов (оценка: символы/4)")
    ratio = 100 * (1 - stats(outputs["agent.txt"])["bytes"] / agent_input_bytes)
    print(f"Agent: на {ratio:.1f}% меньше исходников тех же файлов; частей: {len(chunks)}.")
    print("Для cut-and-paste: agent.txt либо parts/agent-*.txt. Состав и ограничения: index.txt.")


if __name__ == "__main__":
    try:
        main()
    except (OSError, UnicodeError, ValueError) as error:
        print(f"Ошибка: {error}", file=sys.stderr)
        sys.exit(1)
