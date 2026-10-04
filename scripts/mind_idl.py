#!/usr/bin/env python3
"""MIND IDL v0: generates libmind bindings from a WIT subset (docs/idl/README.md).

Usage: mind_idl.py            write libmind/src/idl/<interface>.rs for every idl/*.wit
       mind_idl.py --check    fail if a generated file is missing or out of date
       --root DIR             use DIR instead of the repository root
"""
import re
import sys
from dataclasses import dataclass, field
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
IDL_DIR = ROOT / "idl"
OUT_DIR = ROOT / "libmind" / "src" / "idl"

INTEGERS = {"bool": 1, "u8": 8, "u16": 16, "u32": 32, "u64": 64}
HANDLES = {"memory": "CAP_KIND_MEMORY", "endpoint": "CAP_KIND_ENDPOINT"}
WORD0_START, WORD_BITS = 16, 64  # word 0: method:8 | major:8 | fields


class IdlError(Exception):
    pass


@dataclass
class Field:
    name: str
    type: str  # integer type, or "own<k>" / "borrow<k>"
    word: int = 0
    shift: int = 0

    @property
    def handle(self):
        match = re.fullmatch(r"(own|borrow)<(\w+)>", self.type)
        return (match.group(1), match.group(2)) if match else None

    @property
    def bits(self):
        return INTEGERS[self.type]


@dataclass
class Function:
    name: str
    doc: list
    params: list
    result: str | None  # None, an integer type or option<integer>
    index: int = 0
    result_field: Field | None = None
    handle: Field | None = None
    used: list = field(default_factory=lambda: [0, 0])


@dataclass
class Interface:
    package: str
    name: str
    version: tuple
    doc: list
    functions: list


def snake(name):
    return name.replace("-", "_")


def layout(fields, start=WORD0_START):
    """Places integer fields in declaration order; a field never straddles words. Returns the used-bit masks."""
    used, word, offset = [0, 0], 0, start
    for f in fields:
        if offset + f.bits > WORD_BITS:
            word, offset = word + 1, 0
        if word > 1:
            raise IdlError(f"fields do not fit in two message words at '{f.name}'")
        f.word, f.shift = word, offset
        used[word] |= ((1 << f.bits) - 1) << offset
        offset += f.bits
    return used


def parse(text, source="<idl>"):
    lines = text.splitlines()
    package = name = None
    version, doc, pending, functions, depth = None, [], [], [], 0
    for number, raw in enumerate(lines, 1):
        line = raw.strip()
        where = f"{source}:{number}"
        if not line:
            continue
        if line.startswith("///"):
            pending.append(line[3:].strip())
            continue
        if line.startswith("//"):
            continue
        if match := re.fullmatch(r"package\s+([\w-]+:[\w-]+)@(\d+)\.(\d+)\.(\d+);", line):
            package, version = match.group(1), tuple(int(match.group(i)) for i in (2, 3, 4))
            doc, pending = doc + pending, []
            continue
        if match := re.fullmatch(r"interface\s+([\w-]+)\s*\{", line):
            if name or depth:
                raise IdlError(f"{where}: one interface per file")
            name, depth = match.group(1), 1
            doc, pending = doc + pending, []
            continue
        if line == "}":
            depth -= 1
            continue
        if match := re.fullmatch(r"([\w-]+)\s*:\s*func\((.*)\)\s*(?:->\s*(.+?))?\s*;", line):
            if depth != 1:
                raise IdlError(f"{where}: function outside the interface")
            params = []
            for part in filter(None, (p.strip() for p in match.group(2).split(","))):
                pm = re.fullmatch(r"([\w-]+)\s*:\s*([\w<>-]+)", part)
                if not pm:
                    raise IdlError(f"{where}: bad parameter '{part}'")
                params.append(Field(snake(pm.group(1)), pm.group(2)))
            functions.append(Function(snake(match.group(1)), pending, params, match.group(3)))
            pending = []
            continue
        raise IdlError(f"{where}: unsupported syntax: {line}")
    if not package or not name or depth != 0:
        raise IdlError(f"{source}: needs a package line and one closed interface")
    if version[0] == 0 or version[0] > 255:
        raise IdlError(f"{source}: major version must be 1..255")
    if not 1 <= len(functions) <= 255:
        raise IdlError(f"{source}: 1..255 functions")
    interface = Interface(package, name, version, doc, functions)
    for index, function in enumerate(functions, 1):
        check(function, f"{source}: {function.name}")
        function.index = index
    return interface


def check(function, where):
    integers = []
    for p in function.params:
        if p.handle:
            if p.handle[1] not in HANDLES:
                raise IdlError(f"{where}: unknown handle kind '{p.handle[1]}'")
            if function.handle:
                raise IdlError(f"{where}: at most one capability per message")
            function.handle = p
        elif p.type in INTEGERS:
            integers.append(p)
        else:
            raise IdlError(f"{where}: unsupported parameter type '{p.type}'")
    function.used = layout(integers)
    result = function.result
    if result is not None:
        inner = re.fullmatch(r"option<(\w+)>", result)
        base = inner.group(1) if inner else result
        if base not in INTEGERS:
            raise IdlError(f"{where}: unsupported result type '{result}'")
        function.result_field = Field("value", base)
        layout([function.result_field])


def rust_type(t):
    return {"bool": "bool", "u8": "u8", "u16": "u16", "u32": "u32", "u64": "u64"}[t]


def result_type(function):
    if function.result is None:
        return "()"
    inner = re.fullmatch(r"option<(\w+)>", function.result)
    return f"Option<{rust_type(inner.group(1))}>" if inner else rust_type(function.result)


def encode(f, expr):
    return f"(({expr}) as usize) << {f.shift}"


def decode(f, words):
    value = f"wire::field(&{words}, {f.word}, {f.shift}, {f.bits})"
    return f"{value} != 0" if f.type == "bool" else f"{value} as {rust_type(f.type)}"


def generate(interface, source):
    out = []
    w = out.append
    major, minor, patch = interface.version
    title = snake(interface.name)
    w(f"// Generated by scripts/mind_idl.py from {source}; do not edit.")
    for line in interface.doc:
        w(f"//! {line}" if line else "//!")
    w("#![allow(clippy::all, unused_imports)]")
    w("use crate::abi::*;")
    w("use crate::ipc::{Endpoint, Received};")
    w("use crate::sys::Result;")
    w("use super::wire::{self, Reject};")
    w("")
    w(f'pub const PACKAGE: &str = "{interface.package}";')
    w(f"pub const VERSION: (u8, u8, u8) = ({major}, {minor}, {patch});")
    w(f"const MAJOR: usize = {major};")
    w("")
    # Client side.
    for f in interface.functions:
        for line in f.doc:
            w(f"/// {line}")
        args = ["endpoint: Endpoint"] + [f"{p.name}: {'usize' if p.handle else rust_type(p.type)}" for p in f.params]
        w(f"pub fn {f.name}({', '.join(args)}) -> Result<{result_type(f)}> {{")
        words = [[f"{f.index}", "MAJOR << 8"], []]
        for p in f.params:
            if not p.handle:
                words[p.word].append(encode(p, p.name))
        w(f"    let words = [{' | '.join(words[0])}, {' | '.join(words[1]) or '0'}];")
        if f.handle:
            mode, _ = f.handle.handle
            w(f"    let reply = wire::call(endpoint, words, Some(({f.handle.name}, {'true' if mode == 'own' else 'false'})))?;")
        else:
            w("    let reply = wire::call(endpoint, words, None)?;")
        if f.result is None:
            w("    wire::check_reply(&reply, [0, 0], false).map(drop)")
        else:
            rf = f.result_field
            optional = f.result.startswith("option<")
            used = [0, 0]
            used[rf.word] = ((1 << rf.bits) - 1) << rf.shift
            w(f"    let {'none' if optional else '_'} = wire::check_reply(&reply, [{used[0]:#x}, {used[1]:#x}], {'true' if optional else 'false'})?;")
            value = decode(rf, "reply")
            w(f"    Ok({'if none { None } else { Some(' + value + ') }' if optional else value})")
        w("}")
        w("")
    # Server side.
    w(f"/// A request to the `{interface.name}` interface that passed the receiver's schema check.")
    w("#[derive(Clone, Copy, Debug, PartialEq, Eq)]")
    w("pub enum Request {")
    for f in interface.functions:
        fields = [f"{p.name}: {'usize' if p.handle else rust_type(p.type)}" for p in f.params]
        w(f"    {camel(f.name)}" + (f" {{ {', '.join(fields)} }}," if fields else ","))
    w("}")
    w("")
    w("/// Checks a received message against the schema (MC-2.4): method, major version, unused bits, capability kind.")
    w("/// `cap` is the slot passed to `recv`; an unexpected capability is dropped (MC-2.12).")
    w("pub fn decode(request: &Received, cap: usize) -> core::result::Result<Request, Reject> {")
    w("    let words = request.data;")
    w("    wire::header(request, cap, MAJOR)?;")
    w("    match words[0] & 0xFF {")
    for f in interface.functions:
        handle = f.handle
        kind = HANDLES[handle.handle[1]] if handle else None
        w(f"        {f.index} => {{")
        w(f"            wire::body(request, cap, [{f.used[0]:#x}, {f.used[1]:#x}], {kind if kind else 'CAP_KIND_NONE'}, {'true' if handle else 'false'})?;")
        fields = []
        for p in f.params:
            fields.append(f"{p.name}: cap" if p.handle else f"{p.name}: {decode(p, 'words')}")
        if fields:
            w(f"            Ok(Request::{camel(f.name)} {{ {', '.join(fields)} }})")
        else:
            w(f"            Ok(Request::{camel(f.name)})")
        w("        }")
    w("        _ => { wire::discard(request, cap); Err(Reject::Invalid) }")
    w("    }")
    w("}")
    w("")
    for f in interface.functions:
        if f.result is None:
            w(f"pub fn reply_{f.name}() -> Result<()> {{ wire::reply([0, 0]) }}")
            continue
        rf = f.result_field
        optional = f.result.startswith("option<")
        value_words = ["0", "0"]
        value_words[rf.word] = encode(rf, "value")
        if optional:
            w(f"pub fn reply_{f.name}(value: {result_type(f)}) -> Result<()> {{")
            w(f"    match value {{ None => wire::reply([wire::STATUS_NONE, 0]), Some(value) => wire::reply([{value_words[0]}, {value_words[1]}]) }}")
            w("}")
        else:
            w(f"pub fn reply_{f.name}(value: {result_type(f)}) -> Result<()> {{ wire::reply([{value_words[0]}, {value_words[1]}]) }}")
    w("")
    return "\n".join(out)


def camel(name):
    return "".join(part.capitalize() for part in name.split("_"))


def main(argv):
    global ROOT, IDL_DIR, OUT_DIR
    if "--root" in argv:  # generate for another tree (tests)
        ROOT = Path(argv[argv.index("--root") + 1]).resolve()
        IDL_DIR, OUT_DIR = ROOT / "idl", ROOT / "libmind" / "src" / "idl"
    check_only = "--check" in argv
    stale = []
    names = []
    for path in sorted(IDL_DIR.glob("*.wit")):
        source = path.relative_to(ROOT).as_posix()
        try:
            interface = parse(path.read_text(), source)
        except IdlError as error:
            print(f"error: {error}", file=sys.stderr)
            return 1
        name = snake(interface.name)
        names.append(name)
        target = OUT_DIR / f"{name}.rs"
        text = generate(interface, source)
        if check_only:
            if not target.exists() or target.read_text() != text:
                stale.append(target.relative_to(ROOT).as_posix())
        else:
            target.write_text(text)
    module = "// Generated by scripts/mind_idl.py; do not edit.\n//! Interfaces generated from idl/*.wit (MIND IDL v0).\npub mod wire;\n" + "".join(f"pub mod {n};\n" for n in names)
    mod_path = OUT_DIR / "mod.rs"
    if check_only:
        if not mod_path.exists() or mod_path.read_text() != module:
            stale.append(mod_path.relative_to(ROOT).as_posix())
        if stale:
            print("stale generated files (run scripts/mind_idl.py): " + ", ".join(stale), file=sys.stderr)
            return 1
        print(f"MIND IDL: {len(names)} interface(s) up to date")
        return 0
    mod_path.write_text(module)
    print(f"MIND IDL: generated {len(names)} interface(s)")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
